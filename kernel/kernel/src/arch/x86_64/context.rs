use core::arch::global_asm;
use core::mem;
use core::ptr::addr_of_mut;

use crate::mm::types::VirtAddr;
use crate::sync::irq::IrqDisabled;

use super::percpu;

/// The kernel-mode register context saved to the stack when a thread is switched out.
///
/// This structure does not contain all register values, as only callee-saved registers need to be
/// preserved during explicit [`switch`] calls. Other registers (e.g., during preemption) will be
/// saved in the IRQ handler that triggers the preemption.
// Make sure to keep this structure in sync with the implementation of `do_context_switch`.
#[repr(C)]
pub struct InactiveKernelFrame {
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // This field must always be last as it is pushed implicitly by the call.
    pub rip: u64,
}

pub struct ThreadContext {
    sp: VirtAddr,
    stack_top: VirtAddr,
}

impl ThreadContext {
    /// Creates a new thread context with stack pointer `sp` for entry via `entry_point`.
    ///
    /// This function will set up the stack such that the next time it is switched to via [`switch`],
    /// it will call `entry_point(arg1, arg2)`.
    ///
    /// # Safety
    ///
    /// * `sp` must point to the top of a valid, writable region of memory that will function as the
    ///   stack.
    pub unsafe fn new(
        stack_top: VirtAddr,
        entry_point: extern "C" fn(usize) -> !,
        arg: usize,
    ) -> Self {
        let frame = InactiveKernelFrame {
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: early_thread_start as unsafe extern "C" fn() as usize as u64,
        };

        let mut sp = stack_top;

        unsafe {
            // Keep this return address aligned to a 16-byte boundary, so that the stack will be offset
            // by 8 bytes from 16-byte alignment after returning from `early_thread_start` to the entry
            // point.
            push_data(&mut sp, &entry_point, 16);
            push_data(&mut sp, &arg, 1);
            push_data(&mut sp, &frame, 1);
        }

        Self { sp, stack_top }
    }
}

/// Switches to the kernel-mode stack pointer and register context specified by `new`, storing any
/// necessary existing context to `old` before the switch.
///
/// As far as the caller is concerned, this function will return only when another thread switches
/// back to its context, so this function effectively blocks the caller until it is switched back
/// to. The necessary (callee-saved) registers are pushed to the caller's stack, and then popped
/// from the new stack after the switch, giving the effect of a blocking function call to both the
/// calling thread and the new thread.
///
/// The context saved to the stack is stored in the format of an [`InactiveKernelFrame`].
///
/// # Safety
///
/// * Both `old` and `new` must point to valid [`ThreadContext`] instances that have been previously
///   initialized by a call to `ThreadContext::new`.
/// * Interrupts must be disabled. This function does not explicitly take an `IrqDisabled`
///   parameter as it would leave the instance alive much longer than intended.
pub unsafe fn switch(old: *mut ThreadContext, new: *const ThreadContext) {
    unsafe {
        // Safe by function contract.
        let irq_disabled = IrqDisabled::new();
        (*percpu::current_x64(&irq_disabled).tss.get()).set_rsp0((*new).stack_top);

        do_context_switch(addr_of_mut!((*old).sp), (*new).sp);
    }
}

unsafe fn push_data<T: ?Sized>(sp: &mut VirtAddr, val: &T, align: usize) {
    let size = mem::size_of_val(val);
    *sp = (*sp - size).align_down(align);
    unsafe {
        sp.as_mut_ptr::<u8>()
            .copy_from_nonoverlapping(val as *const _ as *const u8, size);
    }
}

extern "C" {
    fn do_context_switch(old_sp: *mut VirtAddr, new_sp: VirtAddr);
    fn early_thread_start();
}

global_asm!(include_str!("context.s"));
