use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mmu::PAGE_SIZE;
use crate::arch::pmm::BOOTHEAP_BASE;
use crate::kimage;
use crate::mm::bootheap::BootHeap;
use crate::mm::types::PhysFrameNum;

use super::types::PhysAddr;

pub unsafe fn init(mem_map: &[MemoryRange]) {
    todo!()
}
