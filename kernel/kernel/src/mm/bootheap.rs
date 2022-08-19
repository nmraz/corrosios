use core::alloc::Layout;
use core::ops::Range;

use crate::arch::mmu::PageTableSpace;

use super::pt::{PageTableAlloc, PageTableAllocError};
use super::types::{PhysAddr, PhysFrameNum};

pub struct BootHeap {
    base: PhysAddr,
    cur: PhysAddr,
    end: PhysAddr,
}

impl BootHeap {
    pub fn new(range: Range<PhysAddr>) -> Self {
        Self {
            base: range.start,
            cur: range.start,
            end: range.end,
        }
    }

    pub fn used_range(&self) -> Range<PhysAddr> {
        self.base..self.cur
    }

    pub fn alloc_phys(&mut self, layout: Layout) -> PhysAddr {
        let base = self.cur.align_up(layout.align());
        if base > self.end || layout.size() > self.end - base {
            panic!("bootheap exhausted");
        }

        self.cur = base + layout.size();
        base
    }
}

impl PageTableAlloc for BootHeap {
    fn allocate(&mut self) -> Result<PhysFrameNum, PageTableAllocError> {
        Ok(self
            .alloc_phys(Layout::new::<PageTableSpace>())
            .containing_frame())
    }
}
