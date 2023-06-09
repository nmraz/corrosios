use core::marker::PhantomData;

use crate::arch;

use super::resched::ReschedDisabled;

/// A type-level assertion that interrupts are disabled on the current core.
///
/// Whenever an instance of this structure is alive, users can safely assume that interrupts are
/// disbled.
pub struct IrqDisabled {
    _not_send_sync: PhantomData<*const ()>,
    resched_disabled: ReschedDisabled,
}

impl IrqDisabled {
    /// Creates a new instance of the type, asserting that interrupts are actually disabled.
    ///
    /// # Safety
    ///
    /// Interrupts must remain disabled for the duration of the returned object's lifetime.
    ///
    /// # Panics
    ///
    /// This function panics if interrupts are enabled when it is called.
    #[track_caller]
    pub unsafe fn new() -> Self {
        assert!(
            !enabled(),
            "attempted to construct `IrqDisabled` with interrupts enabled"
        );
        unsafe { Self::new_unchecked() }
    }

    /// Creates a new instance of the type without checking whether interrupts are enabled.
    ///
    /// # Safety
    ///
    /// Interrupts must actually be disabled when this function is called and must remain disabled
    /// for the duration of the returned object's lifetime.
    pub unsafe fn new_unchecked() -> Self {
        Self {
            _not_send_sync: PhantomData,
            resched_disabled: unsafe { ReschedDisabled::new_unchecked() },
        }
    }

    pub fn resched_disabled(&self) -> &ReschedDisabled {
        &self.resched_disabled
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

        let ret = f(&IrqDisabled::new_unchecked());

        if prev_state {
            enable();
        }

        ret
    }
}
