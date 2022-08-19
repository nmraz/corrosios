use crate::mm::types::VirtPageNum;

pub const PHYS_MAP_BASE: VirtPageNum = VirtPageNum::new(0xFFFF800000000);
pub const PHYS_MAP_PAGES: usize = 0x1000000;
