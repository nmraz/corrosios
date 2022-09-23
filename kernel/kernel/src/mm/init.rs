use core::cmp;
use core::ops::Range;

use arrayvec::ArrayVec;
use bootinfo::item::{MemoryKind, MemoryRange};
use bootinfo::view::View;
use bootinfo::ItemKind;
use num_utils::div_ceil;

use crate::arch::mm::BOOTHEAP_EARLYMAP_MAX_PAGES;
use crate::arch::mmu::PAGE_SIZE;
use crate::mm::early::{BootHeap, EarlyMapPfnTranslator};
use crate::mm::utils::display_byte_size;
use crate::mm::{physmap, pmm};
use crate::{arch, kimage};

use super::types::{PhysAddr, PhysFrameNum};
use super::{early, utils};

/// # Safety
///
/// * This function must be called only once during initialization
/// * The physical address range passed in `bootinfo_paddr` and `bootinfo_size` must contain a valid
///   bootinfo structure, with correct memory map information
pub unsafe fn init(bootinfo_paddr: PhysAddr, bootinfo_size: usize) {
    // Safety: function contract
    let mut mapper = unsafe { early::get_early_mapper() };

    let bootinfo_pages = div_ceil(bootinfo_size, PAGE_SIZE);
    let bootinfo_ptr = mapper
        .map(bootinfo_paddr.containing_frame(), bootinfo_pages)
        .addr()
        .as_ptr();

    // Safety: the bootinfo has now been identity mapped and is valid by the function contract
    let bootinfo_view = unsafe { View::new(bootinfo_ptr) }.expect("bad bootinfo");
    let mem_map = get_mem_map(bootinfo_view);

    print_mem_info(mem_map);

    let bootinfo_frame_range =
        bootinfo_paddr.containing_frame()..(bootinfo_paddr + bootinfo_size).containing_tail_frame();
    let reserved_ranges = gather_reserved_ranges(bootinfo_frame_range);

    let bootheap_range = largest_usable_range(mem_map, &reserved_ranges);
    let bootheap_pages = bootheap_range.end - bootheap_range.start;

    println!(
        "selected bootheap range: {}-{} ({} pages, {})",
        bootheap_range.start,
        bootheap_range.end,
        bootheap_pages,
        display_byte_size(bootheap_pages * PAGE_SIZE)
    );

    let mut bootheap = BootHeap::new(bootheap_range.start.addr()..bootheap_range.end.addr());
    let bootheap_earlymap_pages = cmp::min(bootheap_pages, BOOTHEAP_EARLYMAP_MAX_PAGES);

    println!(
        "mapping {} bootheap pages for physmap initialization",
        bootheap_earlymap_pages
    );

    mapper.map(bootheap_range.start, bootheap_earlymap_pages);

    unsafe {
        physmap::init(
            mem_map,
            &mut bootheap,
            EarlyMapPfnTranslator::new(
                bootheap_range.start..bootheap_range.start + bootheap_earlymap_pages,
            ),
        );
        pmm::init(mem_map, &reserved_ranges, bootheap);
    }
}

fn print_mem_info(mem_map: &[MemoryRange]) {
    let mut usable_pages = 0;

    println!("physical memory map:");
    for range in mem_map {
        display_range(range);
        if range.kind == MemoryKind::USABLE {
            usable_pages += range.page_count;
        }
    }
    println!(
        "{} pages ({}) usable",
        usable_pages,
        display_byte_size(usable_pages * PAGE_SIZE)
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

fn gather_reserved_ranges(bootinfo_range: Range<PhysFrameNum>) -> ArrayVec<Range<PhysFrameNum>, 5> {
    let mut ret = ArrayVec::new();
    ret.extend([kimage::phys_base()..kimage::phys_end(), bootinfo_range]);
    ret.extend(arch::mm::RESERVED_RANGES);
    ret.sort_unstable_by_key(|range| range.start);
    ret
}

fn largest_usable_range(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
) -> Range<PhysFrameNum> {
    let mut largest: Option<Range<PhysFrameNum>> = None;

    utils::iter_usable_ranges(mem_map, reserved_ranges, |start, end| match &largest {
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
