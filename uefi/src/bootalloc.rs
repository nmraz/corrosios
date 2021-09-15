use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::{self, NonNull};

use crate::table::BootServices;

const MAX_ALIGN: usize = 8;

pub struct BootAlloc<'a> {
    boot_services: &'a BootServices,
}

impl<'a> BootAlloc<'a> {
    pub fn new(boot_services: &'a BootServices) -> Self {
        Self { boot_services }
    }
}

unsafe impl Allocator for BootAlloc<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        assert!(
            layout.align() <= MAX_ALIGN,
            "over-aligned allocations are not supported"
        );

        let p = self
            .boot_services
            .alloc(layout.size())
            .map_err(|_| AllocError)?;

        NonNull::new(ptr::slice_from_raw_parts_mut(p, layout.size())).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, p: NonNull<u8>, _layout: Layout) {
        self.boot_services.free(p.as_ptr());
    }
}
