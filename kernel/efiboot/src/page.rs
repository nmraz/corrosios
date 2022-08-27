use core::mem::MaybeUninit;

use num_utils::div_ceil;
use uefi::table::{AllocMode, BootServices};
use uefi::Result;

pub const PAGE_SIZE: usize = 0x1000;

pub fn alloc_uninit_pages(
    boot_services: &BootServices,
    bytes: usize,
) -> Result<&'static mut [MaybeUninit<u8>]> {
    let pages = div_ceil(bytes, PAGE_SIZE);
    let p = boot_services.alloc_pages(AllocMode::Any, pages)?;
    Ok(unsafe { core::slice::from_raw_parts_mut(p as *mut _, pages * PAGE_SIZE) })
}
