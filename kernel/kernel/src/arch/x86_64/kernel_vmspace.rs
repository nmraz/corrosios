use crate::mm::types::PhysPfn;

pub const PHYS_MAP_BASE: PhysPfn = PhysPfn::new(0xFFFF800000000);
pub const PHYS_MAP_PAGES: usize = 0x400000000000;

pub const KERNEL_IMAGE_SPACE_BASE: PhysPfn = PhysPfn::new(0xFFFFFFFF80000);
pub const KERNEL_IMAGE_SPACE_PAGES: usize = 0x800;
