use core::cmp;
use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};
use bootinfo::view::View;
use bootinfo::ItemKind;

use crate::arch::mmu::PAGE_SIZE;
use crate::arch::pmm::{BOOTHEAP_BASE, BOOTHEAP_EARLYMAP_MAX_PAGES};
use crate::kimage;
use crate::mm::bootheap::BootHeap;
use crate::mm::earlymap::EarlyMapPfnTranslator;
use crate::mm::physmap;

use super::earlymap;
use super::types::{PhysAddr, PhysFrameNum};
use super::utils::div_ceil;

/// # Safety
///
/// * This function must be called only once during initialization
/// * The physical address range passed in `bootinfo_paddr` and `bootinfo_size` must contain a valid
///   bootinfo structure
pub unsafe fn init(bootinfo_paddr: PhysAddr, bootinfo_size: usize) {
    // Safety: function contract
    let mut mapper = unsafe { earlymap::get_early_mapper() };

    let bootinfo_pages = div_ceil(bootinfo_size, PAGE_SIZE);
    let bootinfo_ptr = mapper
        .map(bootinfo_paddr.containing_frame(), bootinfo_pages)
        .addr()
        .as_ptr();

    // Safety: the bootinfo has now been identity mapped and is valid by the function contract
    let bootinfo_view = unsafe { View::new(bootinfo_ptr) }.expect("bad bootinfo");
    let mem_map = get_mem_map(bootinfo_view);

    print_mem_info(mem_map);

    let bootheap_range = largest_usable_range(
        mem_map,
        &[
            PhysFrameNum::new(0)..BOOTHEAP_BASE,
            kimage::phys_base()..kimage::phys_base() + kimage::total_pages(),
            bootinfo_paddr.containing_frame()..(bootinfo_paddr + bootinfo_size).containing_frame(),
        ],
    );

    let bootheap_pages = bootheap_range.end - bootheap_range.start;

    println!(
        "selected bootheap range: {}-{} ({} pages, ~{}M)",
        bootheap_range.start,
        bootheap_range.end,
        bootheap_pages,
        bootheap_pages / 0x100
    );

    let mut bootheap = BootHeap::new(bootheap_range.start.addr()..bootheap_range.end.addr());
    let bootheap_earlymap_pages = cmp::min(bootheap_pages, BOOTHEAP_EARLYMAP_MAX_PAGES);

    println!(
        "mapping {} bootheap pages for physmap initialization",
        bootheap_earlymap_pages
    );

    unsafe {
        mapper.map(bootheap_range.start, bootheap_earlymap_pages);
        println!();
        physmap::init(
            mem_map,
            &mut bootheap,
            EarlyMapPfnTranslator::new(
                bootheap_range.start..bootheap_range.start + bootheap_earlymap_pages,
            ),
        );
    }

    let bootheap_used_range = bootheap.used_range();
    println!(
        "\nbootheap usage: {}K",
        (bootheap_used_range.end - bootheap_used_range.start) / 1024
    );

    todo!()
}

fn print_mem_info(mem_map: &[MemoryRange]) {
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
}

fn get_mem_map(bootinfo: View<'_>) -> &[MemoryRange] {
    let mem_map_item = bootinfo
        .items()
        .find(|item| item.kind() == ItemKind::MEMORY_MAP)
        .expect("no memory map in bootinfo");

    // Safety: we trust the bootinfo
    unsafe { mem_map_item.get_slice() }.expect("invalid bootinfo memory map")
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
