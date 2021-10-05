use core::alloc::{Allocator, GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use uefi::table::{BootServices, BootTableHandle};
use uefi::BootAlloc;

pub fn with<R>(boot_table: &BootTableHandle, f: impl FnOnce() -> R) -> R {
    let _guard = BootServicesGuard::new(boot_table.boot_services());
    f()
}

#[global_allocator]
static ALLOCATOR: GlobalBootAlloc = GlobalBootAlloc;

#[alloc_error_handler]
fn handle_alloc_error(_layout: Layout) -> ! {
    panic!()
}

struct GlobalBootAlloc;

unsafe impl GlobalAlloc for GlobalBootAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BootAlloc::new(unsafe { get_boot_services() })
            .allocate(layout)
            .map_or(ptr::null_mut(), |block| block.as_ptr().cast())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { get_boot_services().free(ptr) }
    }
}

static BOOT_SERVICES: AtomicPtr<BootServices> = AtomicPtr::new(ptr::null_mut());

unsafe fn get_boot_services<'a>() -> &'a BootServices {
    let p = BOOT_SERVICES.load(Ordering::Relaxed);
    unsafe { p.as_ref() }.expect("allocator not available")
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
