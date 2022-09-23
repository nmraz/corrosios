use core::cell::UnsafeCell;
use core::hint;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

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

    pub fn lock(&self) -> SpinGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            hint::spin_loop();
        }

        SpinGuard { owner: self }
    }
}

// Safety: we provide the necessary synchronization around accesses to the stored data when multiple
// threads are involved. We still require `T` itself to be `Send` as the spinlock allows the data
// to be accessed mutably from multiple threads.
unsafe impl<T: Send> Sync for SpinLock<T> {}

unsafe impl<T: Send> Send for SpinLock<T> {}

pub struct SpinGuard<'a, T> {
    owner: &'a SpinLock<T>,
}

impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // Safety: we have exclusive access when the lock is locked
        unsafe { &*self.owner.data.get() }
    }
}

impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // Safety: we have exclusive access when the lock is locked
        unsafe { &mut *self.owner.data.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.owner.locked.store(false, Ordering::Release);
    }
}
