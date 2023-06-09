use core::marker::PhantomData;
use core::ops::Deref;

use crate::arch;

use super::irq;

/// A type-level assertion that rescheduling is disabled on the current core.
///
/// Whenever an instance of this type is alive, users can safely assume that no preemptions or
/// other rescheduling will take place.
pub struct ReschedDisabled {
    _not_send_sync: PhantomData<*const ()>,
}

impl ReschedDisabled {
    pub unsafe fn new_unchecked() -> Self {
        Self {
            _not_send_sync: PhantomData,
        }
    }
}

/// A guard for automatically disabling and re-enabling scheduling on the current core in a given
/// scope.
///
/// This can be `Deref`ed to a [`ReschedDisabled`] to enable access to data and operations that
/// require rescheduling to be disabled on the current core.
pub struct ReschedGuard {
    resched_disabled: ReschedDisabled,
}

impl ReschedGuard {
    pub fn new() -> Self {
        disable();
        unsafe {
            Self {
                resched_disabled: ReschedDisabled::new_unchecked(),
            }
        }
    }
}

impl Deref for ReschedGuard {
    type Target = ReschedDisabled;

    fn deref(&self) -> &ReschedDisabled {
        &self.resched_disabled
    }
}

impl Drop for ReschedGuard {
    fn drop(&mut self) {
        unsafe {
            enable();
        }
    }
}

pub fn disable() {
    arch::cpu::disable_resched();
}

pub unsafe fn enable() {
    // TODO: perform the rescheduling if possible and necessary.
    unsafe {
        let _ = arch::cpu::enable_resched();
    }
}

pub fn enabled() -> bool {
    enabled_in_irq() && irq::enabled()
}

pub fn enabled_in_irq() -> bool {
    arch::cpu::resched_disable_count() == 0
}
