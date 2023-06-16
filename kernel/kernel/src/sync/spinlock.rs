use core::cell::UnsafeCell;
use core::hint;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use super::irq::{self, IrqDisabled};
use super::resched;

/// A lock that protects shared data by spinning until it is available.
///
/// These locks may only be held when interrupts are disabled, to avoid various starvation and
/// latency issues.
pub struct SpinLock<T> {
    data: UnsafeCell<T>,
    raw: RawSpinLock,
}

impl<T> SpinLock<T> {
    /// Creates a new unlocked spinlock holding `value`.
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            raw: RawSpinLock::new(),
        }
    }

    /// Returns a mutable reference to the protected data, without taking the lock.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Acquires the lock, spinning until it is ready if necessary.
    ///
    /// The returned [`SpinLockGuard`] can be used to access the protected data, and will
    /// automatically unlock the spinlock when it exits scope. If this function is called on a core
    /// already holding the lock, it will deadlock.
    ///
    /// The lock may only be held as long as interrupts are disabled, as indicated by the
    /// [`IrqDisabled`] parameter.
    pub fn lock<'a>(&'a self, _irq_disabled: &'a IrqDisabled) -> SpinLockGuard<'a, T> {
        self.raw.lock();
        SpinLockGuard { lock: self }
    }

    /// Disables interrupts, locks the lock and invokes `f` on the protected data.
    pub fn with<R>(&self, f: impl FnOnce(&mut T, &IrqDisabled) -> R) -> R {
        irq::disable_with(|irq_disabled| f(&mut self.lock(irq_disabled), irq_disabled))
    }
}

// Safety: we provide the necessary synchronization around accesses to the stored data when multiple
// threads are involved. We still require `T` itself to be `Send` as the spinlock allows the data
// to be accessed mutably from multiple threads.
unsafe impl<T: Send> Sync for SpinLock<T> {}

unsafe impl<T: Send> Send for SpinLock<T> {}

/// An RAII guard for a locked [`SpinLock`].
///
/// This guard enables access to the protected value and will automatically unlock the spinlock when
/// it goes out of scope.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        // Safety: the raw lock was locked on this core when the object was constructed.
        unsafe { self.lock.raw.unlock() }
    }
}

impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: we have exclusive access whenever the lock is locked.
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        // Safety: we have exclusive access whenever the lock is locked.
        unsafe { &mut *self.lock.data.get() }
    }
}

/// A "raw" spinlock primitive around which higher-level abstractions can be built.
///
/// This structure provides direct `lock()` and `unlock()` methods for interacting with the lock.
/// In general, the higher-level [`SpinLock`] should be used instead.
pub struct RawSpinLock {
    locked: AtomicBool,
}

impl RawSpinLock {
    /// Creates a new, unlocked spinlock.
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    /// Locks the spinlock, spinning (busy waiting) if it is already locked.
    ///
    /// This function will deadlock if the lock is already held by the current core when called.
    pub fn lock(&self) {
        resched::disable();
        while self.locked.swap(true, Ordering::Acquire) {
            hint::spin_loop();
        }
    }

    /// Unlocks the spinlock.
    ///
    /// # Safety
    ///
    /// This function should only be called if the spinlock has previously been acquired by the
    /// current core via a call to `lock()`.
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);

        // Safety: by the function contract, this core has previously called `lock()`, which means
        // that it has called `resched::disable()`.
        unsafe {
            resched::enable();
        }
    }
}
