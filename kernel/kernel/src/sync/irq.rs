use core::marker::PhantomData;

use crate::arch;

/// A type-level assertion that interrupts are disabled
///
/// Whenever an instance of this structure is alive, users can safely assume that interrupts are
/// disbled.
pub struct IrqDisabled {
    _not_send: PhantomData<*const ()>,
}

impl IrqDisabled {
    /// # Safety
    ///
    /// Interrupts must actually be disabled when this function is called and must remain disabled
    /// for the duration of the returned object's lifetime.
    pub unsafe fn new() -> Self {
        Self {
            _not_send: PhantomData,
        }
    }
}

pub fn without<R>(f: impl FnOnce(&IrqDisabled) -> R) -> R {
    unsafe {
        let prev_state = arch::cpu::irq_enabled();
        arch::cpu::disable_irq();

        let ret = f(&IrqDisabled::new());

        if prev_state {
            arch::cpu::enable_irq();
        }

        ret
    }
}
