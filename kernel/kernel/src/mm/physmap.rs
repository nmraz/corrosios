use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::kernel_vmspace::{PHYS_MAP_BASE, PHYS_MAP_PAGES};
use crate::{arch, kimage};

use super::pt::{MappingPointer, PageTable, PageTableAlloc, TranslatePhys};
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

/// Initializes the mapping of all regular physical memory at `PHYS_MAP_BASE`
///
/// # Safety
///
/// * This function must be called only once on the bootstrap processor
/// * The memory range provided in `pt_ident_map` should be identity-mapped
pub unsafe fn init(
    mem_map: &[MemoryRange],
    pt_alloc: &mut impl PageTableAlloc,
    pt_ident_map: Range<PhysFrameNum>,
) {
    // Safety: the function contract guarantees that the physical pages are identity-mapped
    let mut pt = unsafe {
        PageTable::new(
            arch::mmu::kernel_pt_root(),
            IdentPfnTranslator(pt_ident_map),
        )
    };

    // Note: the bootloader is responsible for sorting/coalescing the memory map
    let usable_map = mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE);

    for range in usable_map {
        println!(
            "physmap pages {:#x}-{:#x}",
            range.start_page,
            range.start_page + range.page_count
        );

        assert!(
            range.start_page + range.page_count < PHYS_MAP_PAGES,
            "too much physical memory"
        );

        let pfn = PhysFrameNum::new(range.start_page);
        let mut pointer = MappingPointer::new(pfn_to_physmap(pfn), range.page_count);

        pt.map(
            pt_alloc,
            &mut pointer,
            pfn,
            PageTablePerms::READ | PageTablePerms::WRITE,
        )
        .expect("failed to map physmap region");
    }
}

pub fn paddr_to_physmap(paddr: PhysAddr) -> VirtAddr {
    PHYS_MAP_BASE.addr() + paddr.as_usize()
}

pub fn pfn_to_physmap(pfn: PhysFrameNum) -> VirtPageNum {
    PHYS_MAP_BASE + pfn.as_usize()
}

struct IdentPfnTranslator(Range<PhysFrameNum>);

impl TranslatePhys for IdentPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        if kimage::contains_phys(phys) {
            return kimage::vpn_from_kernel_pfn(phys);
        }

        assert!(
            self.0.contains(&phys),
            "page not covered by early page table identity map"
        );
        VirtPageNum::new(phys.as_usize())
    }
}
