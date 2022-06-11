use core::mem::MaybeUninit;

use uefi::table::{AllocMode, BootServices};
use uefi::Result;

pub const PAGE_SIZE: usize = 0x1000;

pub const fn to_page_count(bytes: usize) -> usize {
    (bytes + PAGE_SIZE - 1) / PAGE_SIZE
}

pub fn alloc_uninit_pages(
    boot_services: &BootServices,
    bytes: usize,
) -> Result<&'static mut [MaybeUninit<u8>]> {
    let pages = to_page_count(bytes);
    let p = boot_services.alloc_pages(AllocMode::Any, pages)?;
    Ok(unsafe { core::slice::from_raw_parts_mut(p as *mut _, pages * PAGE_SIZE) })
}
