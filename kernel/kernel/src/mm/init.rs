use core::ops::Range;
use core::{cmp, slice};

use arrayvec::ArrayVec;
use bootinfo::item::{MemoryKind, MemoryRange};
use bootinfo::view::View;
use bootinfo::ItemKind;
use log::{debug, info, trace};
use num_utils::div_ceil;

use crate::arch::mm::BOOTHEAP_EARLYMAP_MAX_PAGES;
use crate::arch::mmu::PAGE_SIZE;
use crate::bootparse::BootinfoData;
use crate::mm::early::{BootHeap, EarlyMapPfnTranslator};
use crate::mm::utils::display_byte_size;
use crate::mm::{physmap, pmm, vm};
use crate::sync::irq::IrqDisabled;
use crate::{arch, kimage};

use super::early;
use super::types::{PhysAddr, PhysFrameNum};
use super::utils::{is_early_usable, is_usable, iter_usable_ranges};

/// A context structure used across both early and late MM initialization.
pub struct InitContext {
    bootheap: BootHeap,
    reserved_ranges: ReservedRanges,
}

/// Performs early initialization of the memory manager.
///
/// When this function returns, the physmap will be initialized and usable for accessing normal
/// physical memory, but other facilities (frame allocator, heap, VM subsystem) will still be
/// unusable and panic if used.
///
/// The intention is that this function be used to perform the bare minimum needed for access to the
/// bootinfo (necessary for early debugging initialization), and that initialization be resumed with
/// [`init_late`] once that is set up.
///
/// # Safety
///
/// * The physical address range passed in `bootinfo_paddr` and `bootinfo_size` must contain a valid
///   bootinfo structure, with correct memory map information
///
/// # Panics
///
/// Panics if this function is called more than once.
pub unsafe fn init_early(
    bootinfo_paddr: PhysAddr,
    bootinfo_size: usize,
    irq_disabled: &IrqDisabled,
) -> InitContext {
    let mut mapper = early::take_early_mapper();

    unsafe {
        arch::mmu::early_init(irq_disabled);
    }

    let bootinfo_pages = div_ceil(bootinfo_size, PAGE_SIZE);
    let bootinfo_ptr = mapper
        .map(bootinfo_paddr.containing_frame(), bootinfo_pages)
        .addr()
        .as_ptr();

    // Safety: function contract, the physical address has now been mapped to `bootinfo_ptr`
    let bootinfo_slice = unsafe { slice::from_raw_parts(bootinfo_ptr, bootinfo_size) };

    let bootinfo_view = View::new(bootinfo_slice).expect("bad bootinfo");
    let mem_map = get_mem_map(bootinfo_view);

    let bootinfo_frame_range =
        bootinfo_paddr.containing_frame()..(bootinfo_paddr + bootinfo_size).containing_tail_frame();
    let reserved_ranges = gather_reserved_ranges(bootinfo_frame_range);

    let bootheap_range = largest_early_usable_range(mem_map, &reserved_ranges);
    let bootheap_pages = bootheap_range.end - bootheap_range.start;

    let mut bootheap = BootHeap::new(bootheap_range.start.addr()..bootheap_range.end.addr());
    let bootheap_earlymap_pages = cmp::min(bootheap_pages, BOOTHEAP_EARLYMAP_MAX_PAGES);

    mapper.map(bootheap_range.start, bootheap_earlymap_pages);

    unsafe {
        physmap::init(
            mem_map,
            &mut bootheap,
            EarlyMapPfnTranslator::new(
                bootheap_range.start..bootheap_range.start + bootheap_earlymap_pages,
            ),
            irq_disabled,
        );
    }

    InitContext {
        bootheap,
        reserved_ranges,
    }
}

/// Completes initialization of the memory manager previously started by [`init_early`].
///
/// When this function returns, all memory manager facilities (physmap, frame allocator, heap, VM)
/// will be fully initialized and functional.
///
/// # Safety
///
/// The bootinfo provided to this function must match the bootinfo provided to [`init_early`] and
/// correctly describes system the memory map.
///
/// # Panics
///
/// Panics if this function is called more than once.
pub unsafe fn init_late(context: InitContext, bootinfo: &BootinfoData, irq_disabled: &IrqDisabled) {
    let InitContext {
        mut bootheap,
        mut reserved_ranges,
    } = context;

    let mem_map = bootinfo.memory_map();

    print_mem_info(mem_map);

    let bootheap_range = bootheap.range();
    let bootheap_size = bootheap_range.end - bootheap_range.start;

    debug!(
        "bootheap range: {}-{} ({})",
        bootheap_range.start,
        bootheap_range.end,
        display_byte_size(bootheap_size)
    );

    let max_pfn = highest_usable_pfn(mem_map);
    let mut added_free_pages = 0;

    unsafe {
        pmm::init(max_pfn, &mut bootheap, irq_disabled);

        reserve_bootheap(&mut reserved_ranges, bootheap);
        iter_early_usable_ranges(mem_map, &reserved_ranges, |start, end| {
            pmm::add_free_range(start, end, irq_disabled);
            added_free_pages += end - start;
        })
    }

    debug!(
        "initialized PMM with {} free pages ({})",
        added_free_pages,
        display_byte_size(added_free_pages * PAGE_SIZE)
    );

    vm::init();
}
fn get_mem_map(bootinfo: View<'_>) -> &[MemoryRange] {
    let mem_map_item = bootinfo
        .items()
        .find(|item| item.kind() == ItemKind::MEMORY_MAP)
        .expect("no memory map in bootinfo");

    // Safety: we trust the bootinfo
    unsafe { mem_map_item.get_slice() }.expect("invalid bootinfo memory map")
}

type ReservedRanges = ArrayVec<Range<PhysFrameNum>, 5>;

fn gather_reserved_ranges(bootinfo_range: Range<PhysFrameNum>) -> ReservedRanges {
    let mut ret = ReservedRanges::new();
    ret.extend([kimage::phys_base()..kimage::phys_end(), bootinfo_range]);
    ret.extend(arch::mm::RESERVED_RANGES);
    sort_reserved_ranges(&mut ret);
    ret
}

fn reserve_bootheap(reserved_ranges: &mut ReservedRanges, bootheap: BootHeap) {
    let bootheap_used_range = bootheap.used_range();
    debug!(
        "final bootheap usage: {}-{} ({})",
        bootheap_used_range.start,
        bootheap_used_range.end,
        display_byte_size(bootheap_used_range.end - bootheap_used_range.start)
    );

    let bootheap_used_frames = bootheap_used_range.start.containing_frame()
        ..bootheap_used_range.end.containing_tail_frame();

    reserved_ranges.push(bootheap_used_frames);
    sort_reserved_ranges(reserved_ranges);
}

fn sort_reserved_ranges(reserved_ranges: &mut ReservedRanges) {
    reserved_ranges.sort_unstable_by_key(|range| range.start);
}

fn largest_early_usable_range(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
) -> Range<PhysFrameNum> {
    let mut largest: Option<Range<PhysFrameNum>> = None;

    iter_early_usable_ranges(mem_map, reserved_ranges, |start, end| match &largest {
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

pub fn iter_early_usable_ranges(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
    func: impl FnMut(PhysFrameNum, PhysFrameNum),
) {
    iter_usable_ranges(early_usable_ranges(mem_map), reserved_ranges, func);
}

pub fn early_usable_ranges(
    mem_map: &[MemoryRange],
) -> impl DoubleEndedIterator<Item = Range<PhysFrameNum>> + '_ {
    mem_map
        .iter()
        .filter(|range| is_early_usable(range.kind))
        .map(|range| {
            let start = PhysFrameNum::new(range.start_page);
            start..start + range.page_count
        })
}

fn highest_usable_pfn(mem_map: &[MemoryRange]) -> PhysFrameNum {
    mem_map
        .iter()
        .filter(|range| is_usable(range.kind))
        .map(|range| PhysFrameNum::new(range.start_page) + range.page_count)
        .max()
        .expect("no usable memory")
}

fn print_mem_info(mem_map: &[MemoryRange]) {
    let mut usable_pages = 0;

    trace!("physical memory map ({} entries):", mem_map.len());
    for range in mem_map {
        display_range(range);
        if range.kind == MemoryKind::USABLE {
            usable_pages += range.page_count;
        }
    }
    info!(
        "{} pages ({}) usable",
        usable_pages,
        display_byte_size(usable_pages * PAGE_SIZE)
    );
}

fn display_range(range: &MemoryRange) {
    let kind = match range.kind {
        MemoryKind::RESERVED => "reserved",
        MemoryKind::USABLE => "usable",
        MemoryKind::FIRMWARE_BOOT => "firmware (boot)",
        MemoryKind::FIRMWARE_RUNIME => "firmware (runtime)",
        MemoryKind::ACPI_TABLES => "ACPI tables",
        MemoryKind::UNUSABLE => "unusable",
        _ => "other",
    };

    trace!(
        "{:#012x}-{:#012x}: {}",
        range.start_page * PAGE_SIZE,
        (range.start_page + range.page_count) * PAGE_SIZE,
        kind
    );
}
