use core::marker::PhantomData;

use crate::arch;

/// A type-level assertion that interrupts are disabled
///
/// Whenever an instance of this structure is alive, users can safely assume that interrupts are
/// disbled.
pub struct IrqDisabled {
    _not_send_sync: PhantomData<*const ()>,
}

impl IrqDisabled {
    /// # Safety
    ///
    /// Interrupts must actually be disabled when this function is called and must remain disabled
    /// for the duration of the returned object's lifetime.
    pub unsafe fn new() -> Self {
        Self {
            _not_send_sync: PhantomData,
        }
    }
}

/// Queries whether interrupts are enabled on the current processor.
pub fn enabled() -> bool {
    arch::cpu::irq_enabled()
}

/// Disables interrupts on the current processor.
pub fn disable() {
    unsafe {
        arch::cpu::disable_irq();
    }
}

/// Enables interrupts on the current processor.
///
/// # Safety
///
/// The current processor must be in a state that is ready to accept interrupts without
/// races/faults. In particular, this function should not be called when there is an [`IrqDisabled`]
/// live in scope.
pub unsafe fn enable() {
    unsafe {
        arch::cpu::enable_irq();
    }
}

/// Invokes `f` with interrupts disabled, and then restores the previous state.
pub fn disable_with<R>(f: impl FnOnce(&IrqDisabled) -> R) -> R {
    unsafe {
        let prev_state = enabled();
        disable();

        let ret = f(&IrqDisabled::new());

        if prev_state {
            enable();
        }

        ret
    }
}
