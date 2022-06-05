use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mmu::PAGE_SIZE;
use crate::arch::pmm::BOOTHEAP_BASE;
use crate::mm::bootheap::BootHeap;
use crate::mm::types::PhysFrameNum;

pub unsafe fn init(mem_map: &[MemoryRange]) {
    let mut usable_pages = 0;

    println!("\nfirmware memory map:");
    for range in mem_map {
        display_range(range);
        if range.kind == MemoryKind::USABLE {
            usable_pages += range.page_count;
        }
    }

    println!(
        "\n{} pages (~{}M) usable\n",
        usable_pages,
        usable_pages / 0x100
    );

    let kernel_base = PhysFrameNum::new(0x104);
    let bootheap_range = largest_usable_range(
        mem_map,
        &[
            PhysFrameNum::new(0)..BOOTHEAP_BASE,
            kernel_base..kernel_base + 0x100,
        ],
    );

    let bootheap_pages = bootheap_range.end - bootheap_range.start;

    println!(
        "selected bootheap range: {:#x}-{:#x} ({} pages, ~{}M)",
        bootheap_range.start.as_usize(),
        bootheap_range.end.as_usize(),
        bootheap_pages,
        bootheap_pages / 0x100
    );

    let mut bootheap = BootHeap::new(bootheap_range.start.addr()..bootheap_range.end.addr());

    todo!()
}

fn largest_usable_range(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
) -> Range<PhysFrameNum> {
    let mut largest: Option<Range<PhysFrameNum>> = None;

    iter_usable_ranges(mem_map, reserved_ranges, |start, end| match &largest {
        Some(cur_largest) => {
            if end - start > cur_largest.end - cur_largest.start {
                largest = Some(start..end);
            }
        }
        None => {
            largest = Some(start..end);
        }
    });

    largest.expect("no usable memory")
}

fn iter_usable_ranges(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
    mut func: impl FnMut(PhysFrameNum, PhysFrameNum),
) {
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
                func(start, reserved.start);
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
            func(start, end);
        }
    }
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

fn display_range(range: &MemoryRange) {
    let kind = match range.kind {
        MemoryKind::RESERVED => "reserved",
        MemoryKind::USABLE => "usable",
        MemoryKind::FIRMWARE => "firmware",
        MemoryKind::ACPI_TABLES => "ACPI tables",
        MemoryKind::UNUSABLE => "unusable",
        _ => "other",
    };

    println!(
        "{:#012x}-{:#012x}: {}",
        range.start_page * PAGE_SIZE,
        (range.start_page + range.page_count) * PAGE_SIZE,
        kind
    );
}
