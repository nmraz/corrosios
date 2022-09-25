use core::cell::UnsafeCell;
use core::hint;
use core::sync::atomic::{AtomicBool, Ordering};

use super::irq::{self, IrqDisabled};

pub struct SpinLock<T> {
    data: UnsafeCell<T>,
    locked: AtomicBool,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            locked: AtomicBool::new(false),
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T, &IrqDisabled) -> R) -> R {
        irq::disable_with(|irq_disabled| {
            self.with_noirq(irq_disabled, |data| f(data, irq_disabled))
        })
    }

    pub fn with_noirq<R>(&self, _irq_disabled: &IrqDisabled, f: impl FnOnce(&mut T) -> R) -> R {
        while self.locked.swap(true, Ordering::Acquire) {
            hint::spin_loop();
        }

        // Safety: we have exclusive access now that the lock is locked
        let ret = unsafe { f(&mut *self.data.get()) };

        self.locked.store(false, Ordering::Release);

        ret
    }
}

// Safety: we provide the necessary synchronization around accesses to the stored data when multiple
// threads are involved. We still require `T` itself to be `Send` as the spinlock allows the data
// to be accessed mutably from multiple threads.
unsafe impl<T: Send> Sync for SpinLock<T> {}

unsafe impl<T: Send> Send for SpinLock<T> {}
