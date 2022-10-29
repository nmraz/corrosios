use bootinfo::item::{MemoryKind, MemoryRange};
use log::debug;

use crate::arch::mm::{PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};
use crate::arch::mmu::kernel_pt_root;
use crate::mm::types::CacheMode;
use crate::sync::irq::IrqDisabled;

use super::pt::{MappingPointer, PageTable, PageTableAlloc, TranslatePhys};
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

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

    // Note: the bootloader is responsible for sorting/coalescing the memory map
    let usable_map = mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE);

    for range in usable_map {
        debug!(
            "mapping frames {:#x}-{:#x}",
            range.start_page,
            range.start_page + range.page_count
        );

        assert!(
            range.start_page + range.page_count < PHYS_MAP_MAX_PAGES,
            "too much physical memory"
        );

        let pfn = PhysFrameNum::new(range.start_page);
        let mut pointer = MappingPointer::new(pfn_to_physmap(pfn), range.page_count);

        // Safety: our allocator is valid as per function contract, we know that interrupts are
        // disabled, and the function contract guarantees that no other cores are up at the moment.
        unsafe {
            pt.map(
                pt_alloc,
                &mut pointer,
                pfn,
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
