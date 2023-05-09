use core::cell::{RefCell, UnsafeCell};
use core::hint;
use core::sync::atomic::{AtomicU32, Ordering};

use alloc::boxed::Box;
use alloc::sync::Arc;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use log::debug;
use object_name::Name;

use crate::arch::context::{self, ThreadContext};
use crate::err::Result;
use crate::mm::kmap::KernelStack;
use crate::mp::current_percpu;
use crate::sync::irq::{self, IrqDisabled};
use crate::sync::SpinLock;

const STATE_INITIAL: u32 = 0;
const STATE_READY: u32 = 1;
const STATE_RUNNING: u32 = 2;
const STATE_PARKED: u32 = 3;
const STATE_DEAD: u32 = 4;

pub struct Thread {
    sched_ownwer_link: LinkedListLink,
    run_queue_link: LinkedListLink,
    stack: KernelStack,
    state: AtomicU32,
    // Only ever touched during context switches
    arch_context: UnsafeCell<ThreadContext>,
    name: Name,
}

impl Thread {
    pub fn current() -> Arc<Self> {
        irq::disable_with(|irq_disabled| {
            with_cpu_state(irq_disabled, |cpu_state| {
                let current_thread = cpu_state.current_thread.clone().expect("no current thread");
                let current_thread = UnsafeRef::into_raw(current_thread);
                unsafe {
                    Arc::increment_strong_count(current_thread);
                    Arc::from_raw(current_thread)
                }
            })
        })
    }

    pub fn new<F: FnOnce() + Send>(name: &str, entry_fn: F) -> Result<Arc<Self>> {
        let entry_fn_data = Box::into_raw(Box::try_new(entry_fn)?);

        let arg = entry_fn_data as usize;
        let stack = KernelStack::new()?;

        extern "C" fn thread_entry<F: FnOnce()>(data: usize) -> ! {
            unsafe {
                complete_context_switch_handoff_and_enable();
            }
            let entry_fn = unsafe { Box::from_raw(data as *mut F) };
            // Todo: exiting the thread early will probably leak this?
            entry_fn();
            exit_current();
        }

        let arch_context = unsafe { ThreadContext::new(stack.top(), thread_entry::<F>, arg) };

        Ok(Arc::try_new(Thread {
            sched_ownwer_link: LinkedListLink::new(),
            run_queue_link: LinkedListLink::new(),
            stack,
            state: AtomicU32::new(STATE_INITIAL),
            arch_context: UnsafeCell::new(arch_context),
            name: Name::new(name),
        })?)
    }

    pub fn start(self: Arc<Self>) {
        let was_initial = self
            .state
            .compare_exchange(
                STATE_INITIAL,
                STATE_READY,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok();
        assert!(was_initial, "thread already started");

        debug!("starting thread '{}'", self.name());

        irq::disable_with(|irq_disabled| {
            SCHED_THREAD_OWNERS
                .lock(irq_disabled)
                .push_back(Arc::clone(&self));
            let self_ref = unsafe { UnsafeRef::from_raw(Arc::into_raw(self)) };
            with_cpu_state(irq_disabled, |cpu_state| {
                cpu_state.run_queue.push_back(self_ref)
            });
        });
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

unsafe impl Sync for Thread {}

intrusive_adapter!(ThreadSchedOwnerAdapter = Arc<Thread>: Thread { sched_ownwer_link: LinkedListLink });
intrusive_adapter!(ThreadRunQueueAdapter = UnsafeRef<Thread>: Thread { run_queue_link: LinkedListLink });

pub fn start() -> ! {
    let irq_disabled = unsafe { IrqDisabled::new() };
    let new_thread = with_cpu_state(&irq_disabled, |cpu_state| {
        let new_thread = cpu_state.take_ready_thread();
        new_thread.state.store(STATE_RUNNING, Ordering::Relaxed);
        new_thread
    });

    let new_context = new_thread.arch_context.get();
    unsafe {
        begin_context_switch_handoff(HandoffState {
            new_thread,
            thread_to_free: None,
        });
        context::set(new_context);
    }
}

fn exit_current() -> ! {
    irq::disable();
    schedule_common(|_cpu_state, old_thread| {
        old_thread.state.store(STATE_DEAD, Ordering::Relaxed);
        Some(old_thread)
    });
    unsafe {
        hint::unreachable_unchecked();
    }
}

fn preempt() {
    irq::disable();
    schedule_common(|cpu_state, old_thread| {
        old_thread.state.store(STATE_READY, Ordering::Relaxed);
        cpu_state.run_queue.push_back(old_thread);
        None
    });
}

fn schedule_common(
    old_thread_handler: impl FnOnce(&mut CpuStateInner, UnsafeRef<Thread>) -> Option<UnsafeRef<Thread>>,
) {
    let irq_disabled = unsafe { IrqDisabled::new() };
    let (prev_context, new_context, handoff_state) = with_cpu_state(&irq_disabled, |cpu_state| {
        let current_thread = cpu_state
            .current_thread
            .clone()
            .expect("no thread to switch out");
        let thread_to_free = old_thread_handler(cpu_state, current_thread.clone());

        let new_thread = cpu_state.take_ready_thread();
        new_thread.state.store(STATE_RUNNING, Ordering::Relaxed);

        let prev_context = current_thread.arch_context.get();
        let new_context = new_thread.arch_context.get();
        let handoff_state = HandoffState {
            new_thread,
            thread_to_free,
        };

        (prev_context, new_context, handoff_state)
    });

    unsafe {
        begin_context_switch_handoff(handoff_state);
        context::switch(prev_context, new_context);
        complete_context_switch_handoff();
    }
}

unsafe fn complete_context_switch_handoff_and_enable() {
    complete_context_switch_handoff();
    unsafe {
        irq::enable();
    }
}

fn begin_context_switch_handoff(handoff_state: HandoffState) {
    let irq_disabled = unsafe { IrqDisabled::new() };
    with_cpu_state(&irq_disabled, |cpu_state| {
        assert!(
            cpu_state.handoff_state.is_none(),
            "attempted new context switch handoff with existing pending handoff"
        );
        cpu_state.handoff_state = Some(handoff_state);
    });
}

fn complete_context_switch_handoff() {
    let irq_disabled = unsafe { IrqDisabled::new() };
    with_cpu_state(&irq_disabled, |cpu_state| {
        let handoff_state = cpu_state
            .handoff_state
            .take()
            .expect("attempted to complete nonexistent handoff");

        cpu_state.current_thread = Some(handoff_state.new_thread);

        // TODO: is dropping the thread with IRQs disabled safe? Make sure to consider dropping the
        // kernel stack, which could end up calling into the memory manager.
        if let Some(to_free) = handoff_state.thread_to_free {
            unsafe {
                SCHED_THREAD_OWNERS
                    .lock(&irq_disabled)
                    .cursor_mut_from_ptr(UnsafeRef::into_raw(to_free))
                    .remove();
            };
        }
    });
}

pub struct CpuState {
    inner: RefCell<CpuStateInner>,
}

impl CpuState {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(CpuStateInner {
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

fn with_cpu_state<R>(irq_disabled: &IrqDisabled, f: impl FnOnce(&mut CpuStateInner) -> R) -> R {
    f(&mut current_percpu(irq_disabled).sched.inner.borrow_mut())
}

static SCHED_THREAD_OWNERS: SpinLock<LinkedList<ThreadSchedOwnerAdapter>> =
    SpinLock::new(LinkedList::new(ThreadSchedOwnerAdapter::NEW));
