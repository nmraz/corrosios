use core::ptr::{self, NonNull};

use uefi::table::{AllocMode, BootServices};
use uefi::{Result, Status};

pub const PAGE_SIZE: usize = 0x1000;

pub const fn to_page_count(bytes: usize) -> usize {
    (bytes + PAGE_SIZE - 1) / PAGE_SIZE
}

pub fn alloc_pages(boot_services: &BootServices, bytes: usize) -> Result<NonNull<[u8]>> {
    let pages = to_page_count(bytes);
    let p = boot_services.alloc_pages(AllocMode::Any, pages)?;

    NonNull::new(ptr::slice_from_raw_parts_mut(
        p as *mut u8,
        pages * PAGE_SIZE,
    ))
    .ok_or(Status::OUT_OF_RESOURCES)
}
