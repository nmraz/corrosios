use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ops::Range;
use core::slice;

use super::physmap::paddr_to_physmap;
use super::types::PhysAddr;

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

    pub fn alloc_slice<T>(&mut self, count: usize) -> &'static mut [MaybeUninit<T>] {
        unsafe {
            slice::from_raw_parts_mut(
                self.alloc(Layout::array::<T>(count).expect("bootheap allocation too large"))
                    .cast(),
                count,
            )
        }
    }

    pub fn used_range(&self) -> Range<PhysAddr> {
        self.base..self.cur
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        paddr_to_physmap(self.alloc_phys(layout)).as_mut_ptr()
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
