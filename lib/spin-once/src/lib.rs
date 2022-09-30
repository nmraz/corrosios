#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use core::cell::UnsafeCell;
use core::hint;
use core::mem::MaybeUninit;
use core::sync::atomic::{fence, AtomicU8, Ordering};

const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const INITIALIZED: u8 = 2;

/// A cell-like type for storing a value that can only be initialized once.
pub struct Once<T> {
    value: UnsafeCell<MaybeUninit<T>>,
    state: AtomicU8,
}

impl<T> Once<T> {
    /// Creates an uninitialized `Once`.
    pub const fn new() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicU8::new(UNINITIALIZED),
        }
    }

    /// Retrives the contained value if this `Once` has already been initialized.
    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == INITIALIZED {
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// Retrives the contained value or atomically initializes it by invoking `f` and storing its
    /// return value.
    ///
    /// If there are multiple concurrent calls to this function, only one of the callers will be
    /// selected and **only** its `f` will be invoked; the others will wait (spin) until
    /// initialization completes.
    pub fn get_or_init_with(&self, f: impl FnOnce() -> T) -> &T {
        unsafe {
            self.get_or_init_with_raw(move |slot| {
                slot.write(f());
            })
        }
    }

    /// Retrives the contained value or atomically initializes it by invoking `f` on its underlying
    /// storage.
    ///
    /// If there are multiple concurrent calls to this function, only one of the callers will be
    /// selected and **only** its `f` will be invoked; the others will wait (spin) until
    /// initialization completes.
    ///
    /// # Safety
    ///
    /// `f` must completely initialize the contained value.
    pub unsafe fn get_or_init_with_raw(&self, f: impl FnOnce(&mut MaybeUninit<T>)) -> &T {
        if let Some(val) = self.get() {
            return val;
        }

        match self.state.compare_exchange(
            UNINITIALIZED,
            INITIALIZING,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => {}
            Err(INITIALIZED) => {
                fence(Ordering::Acquire);
                return unsafe { self.get_unchecked() };
            }
            Err(INITIALIZING) => {
                while self.state.load(Ordering::Relaxed) == INITIALIZING {
                    hint::spin_loop();
                }
                fence(Ordering::Acquire);
                return unsafe { self.get_unchecked() };
            }
            Err(state) => {
                panic!("unknown state {state}");
            }
        }

        unsafe { self.init_with_unchecked(f) }
    }

    /// Initializes the contained value with `value`.
    ///
    /// This function should be used when there is a single, known initializer at a
    /// statically-determined point in time. To implement racy, first-initializer-wins semantics,
    /// use [`Once::get_or_init_with`] instead.
    ///
    /// # Panics
    ///
    /// Panics if this `Once` is already initialized or is being initialized concurrently.
    #[track_caller]
    #[inline]
    pub fn init(&self, value: T) -> &T {
        // Safety: we initialize the slot
        unsafe {
            self.init_with(move |slot| {
                slot.write(value);
            })
        }
    }

    /// Initializes the contained value by invoking `f` on its underlying storage.
    ///
    /// This function should be used when there is a single, known initializer at a
    /// statically-determined point in time. To implement racy, first-initializer-wins semantics,
    /// use [`Once::get_or_init_with_raw`] instead.
    ///
    /// # Safety
    ///
    /// `f` must completely initialize the contained value.
    ///
    /// # Panics
    ///
    /// Panics if this `Once` is already initialized or is being initialized concurrently.
    #[track_caller]
    #[inline]
    pub unsafe fn init_with(&self, f: impl FnOnce(&mut MaybeUninit<T>)) -> &T {
        if self
            .state
            .compare_exchange(
                UNINITIALIZED,
                INITIALIZING,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_err()
        {
            panic!("attempted to re-initialize `Once`");
        }

        unsafe { self.init_with_unchecked(f) }
    }

    unsafe fn init_with_unchecked(&self, f: impl FnOnce(&mut MaybeUninit<T>)) -> &T {
        let retval = unsafe {
            let ptr = self.value.get();
            f(ptr.as_mut().unwrap());
            self.get_unchecked()
        };
        self.state.store(INITIALIZED, Ordering::Release);
        retval
    }

    unsafe fn get_unchecked(&self) -> &T {
        self.value.get().as_ref().unwrap().assume_init_ref()
    }
}

// Safety: we provide synchronization around the initialization of the contained value and
// ultimately hand out only immutable references to it, so we are `Sync` if it is.
unsafe impl<T: Sync> Sync for Once<T> {}

// Safety: we can be sent as long as the contained value can be.
unsafe impl<T: Send> Send for Once<T> {}
