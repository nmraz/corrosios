use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicPtr, Ordering};

use uefi::{BootServices, BootTableHandle};

pub fn with<R>(boot_table: &BootTableHandle, f: impl FnOnce() -> R) -> R {
    let _guard = BootServicesGuard::new(boot_table.boot_services());
    f()
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[alloc_error_handler]
fn handle_alloc_error(_layout: Layout) -> ! {
    panic!()
}

struct Allocator;

const MAX_ALIGN: usize = 8;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(
            layout.align() <= MAX_ALIGN,
            "over-aligned allocations are not supported"
        );

        get_boot_services()
            .alloc(layout.size())
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        get_boot_services().free(ptr)
    }
}

static BOOT_SERVICES: AtomicPtr<BootServices> = AtomicPtr::new(ptr::null_mut());

unsafe fn get_boot_services<'a>() -> &'a BootServices {
    let ptr = NonNull::new(BOOT_SERVICES.load(Ordering::Relaxed)).expect("allocator not available");
    ptr.as_ref()
}

struct BootServicesGuard;

impl BootServicesGuard {
    fn new(boot_services: *const BootServices) -> Self {
        let old = BOOT_SERVICES.swap(boot_services as *mut _, Ordering::Relaxed);
        assert_eq!(old, ptr::null_mut(), "system table already stashed");

        Self
    }
}

impl Drop for BootServicesGuard {
    fn drop(&mut self) {
        BOOT_SERVICES.store(ptr::null_mut(), Ordering::Relaxed);
    }
}
