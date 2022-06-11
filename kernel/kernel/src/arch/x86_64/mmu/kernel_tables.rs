use crate::kimage;
use crate::mm::types::{PhysFrameNum, VirtAddr};

use super::{PageTableSpace, PAGE_SHIFT, PT_LEVEL_SHIFT};

const MB: usize = 0x100000;

const PT_RANGE: usize = 1 << (PT_LEVEL_SHIFT + PAGE_SHIFT);

// Note: keep in sync with linker script and early mapping in `boot.s`
const KERNEL_MAX: usize = 8 * MB;

const KERNEL_PT_COUNT: usize = KERNEL_MAX / PT_RANGE;

#[no_mangle]
static KERNEL_PML4: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PDPT: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PD: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PTS: [PageTableSpace; KERNEL_PT_COUNT] = [PageTableSpace::NEW; KERNEL_PT_COUNT];

pub fn kernel_pt_root() -> PhysFrameNum {
    kimage::pfn_from_kernel_vpn(VirtAddr::from_ptr(&KERNEL_PML4).containing_page())
}
