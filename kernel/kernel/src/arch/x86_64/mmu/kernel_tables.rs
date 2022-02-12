use core::ptr;

use super::{PageTable, PAGE_SHIFT, PT_LEVEL_SHIFT};

const MB: usize = 0x100000;

const PT_RANGE: usize = 1 << (PT_LEVEL_SHIFT + PAGE_SHIFT);

// Note: keep in sync with linker script
const KERNEL_MAX: usize = 8 * MB;

const KERNEL_PT_COUNT: usize = KERNEL_MAX / PT_RANGE;

#[no_mangle]
static mut KERNEL_PML4: PageTable = PageTable::new();

#[no_mangle]
static mut KERNEL_PDPT: PageTable = PageTable::new();

#[no_mangle]
static mut KERNEL_PD: PageTable = PageTable::new();

#[no_mangle]
static mut KERNEL_PTS: [PageTable; KERNEL_PT_COUNT] = [PageTable::new(); KERNEL_PT_COUNT];

pub fn kernel_pt_root() -> *mut PageTable {
    // Safety: we never create a reference here, only a raw pointer
    unsafe { ptr::addr_of_mut!(KERNEL_PML4) }
}
