use bootinfo::item::MemoryRange;
use itertools::Itertools;
use log::debug;

use crate::arch::mm::{PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};
use crate::arch::mmu::kernel_pt_root;
use crate::mm::types::CacheMode;
use crate::sync::irq::IrqDisabled;

use super::pt::{MappingPointer, PageTable, PageTableAlloc, TranslatePhys};
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};
use super::utils::is_usable;

/// Initializes the mapping of all regular physical memory at `PHYS_MAP_BASE`
///
/// # Safety
///
/// * This function must be called only once on the bootstrap processor
/// * `pt_alloc` must return physical frames usable as as fresh page tables
/// * `pt_mapping` must return correct virtual page numbers for queried frames
pub unsafe fn init(
    mem_map: &[MemoryRange],
    pt_alloc: &mut impl PageTableAlloc,
    pt_mapping: impl TranslatePhys,
    _irq_disabled: &IrqDisabled,
) {
    // Safety: the function contract guarantees that `pt_mapping` can be used here
    let mut pt = unsafe { PageTable::new(kernel_pt_root(), pt_mapping) };

    // Note: the bootloader is responsible for sorting the memory map
    let usable_map = mem_map
        .iter()
        .filter(|range| is_usable(range.kind))
        .map(|range| {
            let start = PhysFrameNum::new(range.start_page);
            (start, start + range.page_count)
        })
        .coalesce(|(cur_start, cur_end), (next_start, next_end)| {
            if cur_end == next_start {
                Ok((cur_start, next_end))
            } else {
                Err(((cur_start, cur_end), (next_start, next_end)))
            }
        });

    for (start, end) in usable_map {
        debug!("mapping frames {}-{}", start, end);

        assert!(
            end.as_usize() < PHYS_MAP_MAX_PAGES,
            "too much physical memory"
        );

        let mut pointer = MappingPointer::new(pfn_to_physmap(start), end - start);

        // Safety: our allocator is valid as per function contract, we know that interrupts are
        // disabled, and the function contract guarantees that no other cores are up at the moment.
        unsafe {
            pt.map(
                pt_alloc,
                &mut pointer,
                start,
                PageTablePerms::READ | PageTablePerms::WRITE,
                CacheMode::WriteBack,
            )
            .expect("failed to map physmap region");
        }
    }
}

pub fn paddr_to_physmap(paddr: PhysAddr) -> VirtAddr {
    paddr.to_virt(pfn_to_physmap)
}

pub fn pfn_to_physmap(pfn: PhysFrameNum) -> VirtPageNum {
    PHYS_MAP_BASE + pfn.as_usize()
}

pub fn physmap_to_pfn(vpn: VirtPageNum) -> PhysFrameNum {
    assert!((PHYS_MAP_BASE..PHYS_MAP_BASE + PHYS_MAP_MAX_PAGES).contains(&vpn));
    PhysFrameNum::new(vpn - PHYS_MAP_BASE)
}

pub struct PhysmapPfnTranslator;

impl TranslatePhys for PhysmapPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        pfn_to_physmap(phys)
    }
}
