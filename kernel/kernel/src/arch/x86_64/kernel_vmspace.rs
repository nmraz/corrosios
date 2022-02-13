use crate::mm::types::VirtPageNum;

pub const PHYS_MAP_BASE: VirtPageNum = VirtPageNum::new(0xFFFF800000000);
pub const PHYS_MAP_PAGES: usize = 0x400000000000;

pub const KERNEL_IMAGE_SPACE_BASE: VirtPageNum = VirtPageNum::new(0xFFFFFFFF80000);
pub const KERNEL_IMAGE_SPACE_PAGES: usize = 0x800;

pub const BOOTINFO_SPACE_BASE: VirtPageNum = VirtPageNum::new(0xFFFFFFFF80800);
