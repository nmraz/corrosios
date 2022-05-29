use core::alloc::Layout;
use core::cmp;
use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};
use itertools::Itertools;

use crate::arch::mmu::PAGE_SIZE;
use crate::mm::utils::div_ceil;

use super::types::PhysFrameNum;
use super::utils::align_up;

pub struct BootHeap {
    bitmap: &'static mut [u8],
}

impl BootHeap {
    pub fn new(mem_map: &[MemoryRange], reserved_ranges: &[Range<PhysFrameNum>]) -> Self {
        let bitmap = alloc_bitmap(mem_map, reserved_ranges);
        Self { bitmap }
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        assert!(layout.size() > 0, "zero-sized bootheap allocation");
        assert!(
            layout.align() <= PAGE_SIZE,
            "unsupported alignment for bootheap allocation"
        );

        todo!()
    }

    pub unsafe fn dealloc(&mut self, p: *mut u8, layout: Layout) {
        todo!()
    }

    pub fn finish(self) -> &'static [u8] {
        self.bitmap
    }
}

fn alloc_bitmap(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
) -> &'static mut [u8] {
    let bitmap_size =
        (max_usable_pfn(mem_map).as_usize() + u8::BITS as usize - 1) / u8::BITS as usize;
    let bitmap_pages = div_ceil(bitmap_size, PAGE_SIZE);

    println!(
        "bitmap size: {} bytes ({} pages)",
        bitmap_size, bitmap_pages
    );

    let mut reserved_ranges = reserved_ranges.iter().peekable();

    'outer: for Range { mut start, end } in usable_ranges(mem_map) {
        // Chop up our usable range based on the reserved ranges that intersect it. This loop should
        // always consume all reserved ranges contained in `[0, end)`.
        while let Some(reserved) = reserved_ranges.peek().copied() {
            assert!(reserved.start <= reserved.end);

            if reserved.start >= end || reserved.end < start {
                // The next reserved range doesn't intersect us at all, so we're done here; just
                // make sure to report the remaining usable range below.
                break;
            }

            // Beyond this point: `reserved.start < end && reserved.end >= start`.

            if reserved.start > start {
                // We have a gap before the reserved range, report it.
                print_usable_range(start, reserved.start);
            }
            start = reserved.end;

            if start <= end {
                // We're done with this reserved range now.
                reserved_ranges.next();
            }

            if start >= end {
                // We've covered all of the original usable range, try the next one.
                continue 'outer;
            }
        }

        // Deal with the tail/non-intersecting portion of the range.
        if start < end {
            print_usable_range(start, end);
        }
    }

    todo!()
}

fn print_usable_range(start: PhysFrameNum, end: PhysFrameNum) {
    println!("usable: {:#x}-{:#x}", start.as_usize(), end.as_usize());
}

fn max_usable_pfn(mem_map: &[MemoryRange]) -> PhysFrameNum {
    // Note: we depend on `mem_map` being sorted here
    usable_ranges(mem_map)
        .next_back()
        .expect("no usable memory")
        .end
}

fn usable_ranges(
    mem_map: &[MemoryRange],
) -> impl DoubleEndedIterator<Item = Range<PhysFrameNum>> + '_ {
    mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE)
        .map(|range| {
            let start = PhysFrameNum::new(range.start_page);
            start..start + range.page_count
        })
}
