use core::ops::Range;

use crate::mm::types::PhysFrameNum;
use crate::mm::types::VirtPageNum;

pub const KERNEL_ASPACE_BASE: VirtPageNum = VirtPageNum::new(0xFFFF800000000);
pub const KERNEL_ASPACE_END: VirtPageNum = VirtPageNum::new(0x10000000000000);

// Always leave the low 2MiB unmapped to catch errors.
pub const LOW_ASPACE_BASE: VirtPageNum = VirtPageNum::new(0x200);
pub const LOW_ASPACE_END: VirtPageNum = VirtPageNum::new(0x800000000);

pub const PHYS_MAP_BASE: VirtPageNum = VirtPageNum::new(0xFFFF800000000);
// 64TiB
pub const PHYS_MAP_MAX_PAGES: usize = 0x400000000;

pub const EARLY_MAP_PT_PAGES: usize = 10;
// 1GiB should be enough for early physmap page tables
pub const BOOTHEAP_EARLYMAP_MAX_PAGES: usize = 0x40000;

// Keep the first MiB of physical memory clear to avoid firmware quirks and leave room to bootstrap
// APs later.
const LOWMEM_LIMIT: PhysFrameNum = PhysFrameNum::new(0x100);

pub const RESERVED_RANGES: [Range<PhysFrameNum>; 1] = [PhysFrameNum::new(0)..LOWMEM_LIMIT];
