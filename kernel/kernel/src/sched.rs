use core::cell::{RefCell, UnsafeCell};

use alloc::boxed::Box;
use alloc::sync::Arc;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use object_name::Name;

use crate::arch::context::{self, ThreadContext};
use crate::err::Result;
use crate::mm::kmap::KernelStack;
use crate::mp::current_percpu;
use crate::sync::irq::{self, IrqDisabled};

pub struct Thread {
    run_queue_link: LinkedListLink,
    stack: KernelStack,
    // Only ever touched during context switches
    arch_context: UnsafeCell<ThreadContext>,
    name: Name,
}

impl Thread {
    pub fn new<F: FnOnce() + Send>(name: &str, entry_fn: F) -> Result<Arc<Self>> {
        let entry_fn_data = Box::into_raw(Box::try_new(entry_fn)?);

        let arg = entry_fn_data as usize;
        let stack = KernelStack::new()?;

        extern "C" fn thread_entry<F: FnOnce()>(data: usize) -> ! {
            complete_context_switch_handoff();
            let entry_fn = unsafe { Box::from_raw(data as *mut F) };
            // Todo: exiting the thread early will probably leak this?
            entry_fn();
            todo!("exit here");
        }

        let arch_context = unsafe { ThreadContext::new(stack.top(), thread_entry::<F>, arg) };

        Ok(Arc::try_new(Thread {
            run_queue_link: LinkedListLink::new(),
            stack,
            arch_context: UnsafeCell::new(arch_context),
            name: Name::new(name),
        })?)
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

unsafe impl Sync for Thread {}

intrusive_adapter!(ThreadRunQueueAdapter = Arc<Thread>: Thread { run_queue_link: LinkedListLink });

unsafe fn switch_to_thread(new_thead: Arc<Thread>) {
    assert!(
        !irq::enabled(),
        "attempted to perform context switch with interrupts enabled"
    );
    let irq_disabled = unsafe { IrqDisabled::new() };

    let (prev_context, new_context) = with_cpu_state(&irq_disabled, |cpu_state| {
        let prev_thread = cpu_state
            .current_thread
            .take()
            .expect("no thread to switch out");
        let prev_context = prev_thread.arch_context.get();
        let new_context = new_thead.arch_context.get();
        cpu_state.current_thread = Some(new_thead);
        (prev_context, new_context)
    });

    unsafe {
        begin_context_switch_handoff();
        context::switch(prev_context, new_context);
        complete_context_switch_handoff();
    }
}

fn begin_context_switch_handoff() {
    assert!(!irq::enabled());
    let irq_disabled = unsafe { IrqDisabled::new() };
    with_cpu_state(&irq_disabled, |cpu_state| {
        assert!(
            !cpu_state.in_handoff,
            "attempted new context switch handoff with existing pending handoff"
        );
        cpu_state.in_handoff = true;
    });
}

fn complete_context_switch_handoff() {
    assert!(
        !irq::enabled(),
        "attempted to complete context switch handoff with interrupts enabled"
    );
    {
        let irq_disabled = unsafe { IrqDisabled::new() };
        with_cpu_state(&irq_disabled, |cpu_state| {
            assert!(
                cpu_state.in_handoff,
                "attempted to complete nonexistent handoff"
            );
            cpu_state.in_handoff = false;
        });
    }
}

pub struct CpuState {
    inner: RefCell<CpuStateInner>,
}

impl CpuState {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(CpuStateInner {
                current_thread: None,
                run_queue: LinkedList::new(ThreadRunQueueAdapter::new()),
                in_handoff: false,
            }),
        }
    }
}

struct CpuStateInner {
    current_thread: Option<Arc<Thread>>,
    run_queue: LinkedList<ThreadRunQueueAdapter>,
    in_handoff: bool,
}

fn with_cpu_state<R>(irq_disabled: &IrqDisabled, f: impl FnOnce(&mut CpuStateInner) -> R) -> R {
    f(&mut current_percpu(irq_disabled).sched.inner.borrow_mut())
}
