use crate::mm::types::VirtPageNum;

pub const PHYS_MAP_BASE: VirtPageNum = VirtPageNum::new(0xFFFF800000000);

// 64TiB
pub const PHYS_MAP_MAX_PAGES: usize = 400000000;
