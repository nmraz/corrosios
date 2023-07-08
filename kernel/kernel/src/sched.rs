use core::cell::UnsafeCell;
use core::hint;
use core::sync::atomic::{AtomicU32, Ordering};

use alloc::boxed::Box;
use alloc::sync::Arc;
use atomic_refcell::AtomicRefCell;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use log::{debug, trace};
use object_name::Name;

use crate::arch::context::ThreadContext as ArchContext;
use crate::arch::{self, cpu};
use crate::err::Result;
use crate::mm::kmap::KernelStack;
use crate::mm::types::VirtAddr;
use crate::mm::vm::low_aspace::{self, LowAddrSpace};
use crate::mp::current_percpu;
use crate::sync::irq::{self, IrqDisabled};
use crate::sync::resched::{ReschedDisabled, ReschedGuard};
use crate::sync::{resched, SpinLock};

const STATE_READY: u32 = 1;
const STATE_RUNNING: u32 = 2;
const STATE_PARKED: u32 = 3;
const STATE_DEAD: u32 = 4;

struct Context {
    // Only ever touched during context switches
    arch: UnsafeCell<ArchContext>,
    addr_space: Option<Arc<LowAddrSpace>>,
}

pub struct Thread {
    sched_ownwer_link: LinkedListLink,
    run_queue_link: LinkedListLink,
    state: AtomicU32,
    stack: KernelStack,
    context: Context,
    name: Name,
}

impl Thread {
    pub fn current() -> Option<Arc<Self>> {
        with_cpu_state(&ReschedGuard::new(), |cpu_state| {
            cpu_state.current_thread.clone().map(|current_thread| {
                let current_thread = UnsafeRef::into_raw(current_thread);
                unsafe {
                    Arc::increment_strong_count(current_thread);
                    Arc::from_raw(current_thread)
                }
            })
        })
    }

    pub fn spawn<F: FnOnce() + Send + 'static>(name: &str, entry_fn: F) -> Result<Arc<Self>> {
        let thread = Self::new(name, entry_fn)?;

        debug!("starting thread '{}'", name);

        irq::disable_with(|irq_disabled| {
            let thread_ref = unsafe { UnsafeRef::from_raw(Arc::as_ptr(&thread)) };
            SCHED_THREAD_OWNERS
                .lock(irq_disabled)
                .push_back(thread.clone());
            with_cpu_state_mut(irq_disabled, |cpu_state| {
                cpu_state.run_queue.push_back(thread_ref)
            });
        });

        Ok(thread)
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn stack(&self) -> &KernelStack {
        &self.stack
    }

    pub fn addr_space(&self) -> Option<&Arc<LowAddrSpace>> {
        self.context.addr_space.as_ref()
    }

    fn new<F: FnOnce() + Send + 'static>(name: &str, entry_fn: F) -> Result<Arc<Self>> {
        let entry_fn_data = Box::into_raw(Box::try_new(entry_fn)?);
        let arg = entry_fn_data as usize;
        let stack = KernelStack::new()?;
        extern "C" fn thread_entry<F: FnOnce()>(data: usize) -> ! {
            unsafe {
                complete_context_switch_handoff_and_enable();
            }

            // Make sure we drop `entry_fn` before abruptly exiting the thread.
            {
                let entry_fn = *unsafe { Box::from_raw(data as *mut F) };
                entry_fn();
            }

            exit_current();
        }

        let arch_context = unsafe { ArchContext::new(stack.top(), thread_entry::<F>, arg) };
        let thread = Arc::try_new(Self {
            sched_ownwer_link: LinkedListLink::new(),
            run_queue_link: LinkedListLink::new(),
            state: AtomicU32::new(STATE_READY),
            stack,
            context: Context {
                arch: UnsafeCell::new(arch_context),
                addr_space: None,
            },
            name: Name::new(name),
        })?;

        Ok(thread)
    }
}

unsafe impl Sync for Thread {}

intrusive_adapter!(ThreadSchedOwnerAdapter = Arc<Thread>: Thread { sched_ownwer_link: LinkedListLink });
intrusive_adapter!(ThreadRunQueueAdapter = UnsafeRef<Thread>: Thread { run_queue_link: LinkedListLink });

/// Starts the scheduler on the current core, creating the idle thread and switching to the next
/// ready thread.
///
/// This function expects to be called with interrupts disabled, and will enable them when threads
/// start running.
///
/// # Safety
///
/// This function must be called at most once per core, in a state where it is safe to enable
/// interrupts.
pub unsafe fn start() -> ! {
    let irq_disabled = unsafe { IrqDisabled::new() };
    let new_thread = with_cpu_state_mut(&irq_disabled, |cpu_state| {
        let new_thread = cpu_state.take_ready_thread();
        new_thread.state.store(STATE_RUNNING, Ordering::Relaxed);
        new_thread
    });

    with_cpu_state_mut(&irq_disabled, |cpu_state| {
        let idle_thread =
            Thread::new("idle", || cpu::idle_loop()).expect("failed to create idle thread");
        cpu_state.idle_thread = Some(unsafe { UnsafeRef::from_raw(Arc::into_raw(idle_thread)) });
    });

    unsafe {
        begin_context_switch_handoff(new_thread.clone(), None);
        set_context(&new_thread.context);
    }
}

fn exit_current() -> ! {
    assert!(
        resched::enabled(),
        "attempted to exit thread with rescheduling disabled"
    );

    irq::disable();
    schedule_common(|_cpu_state, old_thread| {
        old_thread.state.store(STATE_DEAD, Ordering::Relaxed);
        Some(old_thread)
    });
    unsafe {
        hint::unreachable_unchecked();
    }
}

fn schedule_common(
    old_thread_handler: impl FnOnce(&mut CpuStateInner, UnsafeRef<Thread>) -> Option<UnsafeRef<Thread>>,
) {
    let irq_disabled = unsafe { IrqDisabled::new() };
    let (old_thread, new_thread, thread_to_free) = with_cpu_state_mut(&irq_disabled, |cpu_state| {
        let current_thread = cpu_state
            .current_thread
            .clone()
            .expect("no thread to switch out");

        check_current_thread_stack(&current_thread);

        let thread_to_free = old_thread_handler(cpu_state, current_thread.clone());
        let new_thread = cpu_state.take_ready_thread();
        new_thread.state.store(STATE_RUNNING, Ordering::Relaxed);

        (current_thread, new_thread, thread_to_free)
    });

    unsafe {
        begin_context_switch_handoff(new_thread.clone(), thread_to_free);
        switch_context(&old_thread.context, &new_thread.context);
        complete_context_switch_handoff();
    }
}

fn check_current_thread_stack(current_thread: &Thread) {
    let on_stack = 0;
    let stack_addr = VirtAddr::from_ptr(&on_stack);

    let stack_top = current_thread.stack().top();
    let stack_bottom = current_thread.stack().bottom();
    if !(stack_bottom..stack_top).contains(&stack_addr) {
        panic!("attempted to switch out thread '{}' on wrong stack: found pointer {stack_addr}, expected range {stack_bottom}-{stack_top}", current_thread.name());
    }
}

unsafe fn switch_context(old_context: &Context, new_context: &Context) {
    unsafe {
        set_common_context(Some(old_context), new_context);
        arch::context::switch(old_context.arch.get(), new_context.arch.get());
    }
}

unsafe fn set_context(context: &Context) -> ! {
    unsafe {
        set_common_context(None, context);
        arch::context::set(context.arch.get())
    }
}

unsafe fn set_common_context(old_context: Option<&Context>, new_context: &Context) {
    unsafe {
        let resched_disabled = ReschedDisabled::new_unchecked();
        low_aspace::switch_to(
            &resched_disabled,
            old_context.and_then(|context| context.addr_space.as_deref()),
            new_context.addr_space.as_deref(),
        );
    }
}

unsafe fn complete_context_switch_handoff_and_enable() {
    complete_context_switch_handoff();
    unsafe {
        irq::enable();
    }
}

fn begin_context_switch_handoff(
    new_thread: UnsafeRef<Thread>,
    thread_to_free: Option<UnsafeRef<Thread>>,
) {
    let irq_disabled = unsafe { IrqDisabled::new() };
    trace!("switching to thread '{}'", new_thread.name());
    with_cpu_state_mut(&irq_disabled, |cpu_state| {
        assert!(
            cpu_state.handoff_state.is_none(),
            "attempted new context switch handoff with existing pending handoff"
        );
        cpu_state.handoff_state = Some(HandoffState {
            new_thread,
            thread_to_free,
        });
    });
}

fn complete_context_switch_handoff() {
    let irq_disabled = unsafe { IrqDisabled::new() };
    with_cpu_state_mut(&irq_disabled, |cpu_state| {
        let handoff_state = cpu_state
            .handoff_state
            .take()
            .expect("attempted to complete nonexistent handoff");

        cpu_state.current_thread = Some(handoff_state.new_thread.clone());

        // TODO: is dropping the thread with IRQs disabled safe? Make sure to consider dropping the
        // kernel stack, which could end up calling into the memory manager.
        if let Some(to_free) = handoff_state.thread_to_free {
            let thread = unsafe {
                SCHED_THREAD_OWNERS
                    .lock(&irq_disabled)
                    .cursor_mut_from_ptr(UnsafeRef::into_raw(to_free))
                    .remove()
                    .unwrap()
            };

            debug!(
                "dropping sched owner for thread '{}', strong count {}",
                thread.name(),
                Arc::strong_count(&thread)
            );
        }

        trace!(
            "finished switching to '{}'",
            handoff_state.new_thread.name()
        );
    });
}

pub struct CpuState {
    inner: AtomicRefCell<CpuStateInner>,
}

impl CpuState {
    pub fn new() -> Self {
        Self {
            inner: AtomicRefCell::new(CpuStateInner {
                current_thread: None,
                idle_thread: None,
                run_queue: LinkedList::new(ThreadRunQueueAdapter::new()),
                handoff_state: None,
            }),
        }
    }
}

struct HandoffState {
    new_thread: UnsafeRef<Thread>,
    thread_to_free: Option<UnsafeRef<Thread>>,
}

struct CpuStateInner {
    current_thread: Option<UnsafeRef<Thread>>,
    idle_thread: Option<UnsafeRef<Thread>>,
    run_queue: LinkedList<ThreadRunQueueAdapter>,
    handoff_state: Option<HandoffState>,
}

impl CpuStateInner {
    #[track_caller]
    fn take_ready_thread(&mut self) -> UnsafeRef<Thread> {
        self.run_queue
            .pop_front()
            .or_else(|| self.idle_thread.clone())
            .expect("no threads ready")
    }
}

fn with_cpu_state_mut<R>(irq_disabled: &IrqDisabled, f: impl FnOnce(&mut CpuStateInner) -> R) -> R {
    assert!(
        resched::enabled_in_irq(),
        "attempted to mutate scheduler state with rescheduling disabled"
    );

    f(&mut current_percpu(irq_disabled.resched_disabled())
        .sched
        .inner
        .borrow_mut())
}

fn with_cpu_state<R>(resched_disabled: &ReschedDisabled, f: impl FnOnce(&CpuStateInner) -> R) -> R {
    f(&current_percpu(resched_disabled).sched.inner.borrow())
}

static SCHED_THREAD_OWNERS: SpinLock<LinkedList<ThreadSchedOwnerAdapter>> =
    SpinLock::new(LinkedList::new(ThreadSchedOwnerAdapter::NEW));
