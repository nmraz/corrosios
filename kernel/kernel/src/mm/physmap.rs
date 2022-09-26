use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mm::{PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};

use super::pt::{MappingPointer, PageTable, PageTableAlloc, TranslatePhys};
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

/// Initializes the mapping of all regular physical memory at `PHYS_MAP_BASE`
///
/// # Safety
///
/// * This function must be called only once on the bootstrap processor
/// * The kernel page tables should not be touched (e.g., by interrupts) for the duration of this
///   function
/// * `pt_alloc` must return physical frames usable as as fresh page tables
/// * `pt_mapping` must return correct virtual page numbers for queried frames
pub unsafe fn init(
    mem_map: &[MemoryRange],
    pt_alloc: &mut impl PageTableAlloc,
    pt_mapping: impl TranslatePhys,
) {
    // Safety: the function contract guarantees that `pt_mapping` can be used here
    let mut pt = unsafe { PageTable::current_kernel(pt_mapping) };

    // Note: the bootloader is responsible for sorting/coalescing the memory map
    let usable_map = mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE);

    for range in usable_map {
        println!(
            "physmap: mapping frames {:#x}-{:#x}",
            range.start_page,
            range.start_page + range.page_count
        );

        assert!(
            range.start_page + range.page_count < PHYS_MAP_MAX_PAGES,
            "too much physical memory"
        );

        let pfn = PhysFrameNum::new(range.start_page);
        let mut pointer = MappingPointer::new(pfn_to_physmap(pfn), range.page_count);

        // Safety: function contract
        unsafe {
            pt.map(
                pt_alloc,
                &mut pointer,
                pfn,
                PageTablePerms::READ | PageTablePerms::WRITE,
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

pub fn physmap_to_paddr(vaddr: VirtAddr) -> PhysAddr {
    vaddr.to_phys(physmap_to_pfn)
}

pub fn physmap_to_pfn(vpn: VirtPageNum) -> PhysFrameNum {
    assert!((PHYS_MAP_BASE..PHYS_MAP_BASE + PHYS_MAP_MAX_PAGES).contains(&vpn));
    PhysFrameNum::new(vpn - PHYS_MAP_BASE)
}
