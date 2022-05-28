use bootinfo::item::{MemoryKind, MemoryRange};
use bootinfo::view::View;
use bootinfo::{ItemHeader, ItemKind};

use crate::arch;
use crate::arch::kernel_vmspace::{PHYS_MAP_BASE, PHYS_MAP_PAGES, PHYS_MAP_PT_PAGES};
use crate::arch::mmu::{PageTable, PAGE_SIZE};

use super::earlymap::{self, BumpPageTableAlloc, EarlyMapper, NoopGather};
use super::pt::MappingPointer;
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};
use super::utils::align_up;

const BOOTINFO_PT_PAGES: usize = 10;

static PHYS_MAP_PT_SPACE: [PageTable; PHYS_MAP_PT_PAGES + BOOTINFO_PT_PAGES] =
    [PageTable::EMPTY; PHYS_MAP_PT_PAGES + BOOTINFO_PT_PAGES];

/// # Safety
///
/// * This function must be called only once on the bootstrap processor
/// * The kernel root page table should refer only to page tables allocated from within the kernel
/// image
pub unsafe fn init(bootinfo_paddr: PhysAddr) {
    let mut alloc = BumpPageTableAlloc::from_kernel_space(&PHYS_MAP_PT_SPACE);

    // Safety: function contract
    let mut mapper =
        unsafe { earlymap::make_early_mapper(arch::mmu::kernel_pt_root(), &mut alloc) };

    let view_size = {
        let view = unsafe { ident_map_bootinfo(&mut mapper, bootinfo_paddr) };
        init_inner(&mut mapper, view);
        view.total_size()
    };

    unsafe { ident_unmap_bootinfo(&mut mapper, bootinfo_paddr, view_size) };
}

pub fn paddr_to_physmap(paddr: PhysAddr) -> VirtAddr {
    PHYS_MAP_BASE.addr() + paddr.as_usize()
}

pub fn pfn_to_physmap(pfn: PhysFrameNum) -> VirtPageNum {
    PHYS_MAP_BASE + pfn.as_usize()
}

fn init_inner(mapper: &mut EarlyMapper<'_>, bootinfo: View<'_>) {
    let mem_map = get_mem_map(bootinfo);

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

        mapper
            .map(
                &mut pointer,
                pfn,
                PageTablePerms::READ | PageTablePerms::WRITE,
            )
            .expect("failed to map physmap region");
    }
}

fn get_mem_map(bootinfo: View<'_>) -> &[MemoryRange] {
    let mem_map_item = bootinfo
        .items()
        .find(|item| item.kind() == ItemKind::MEMORY_MAP)
        .expect("no memory map in bootinfo");

    // Safety: we trust the bootinfo
    unsafe { mem_map_item.get_slice() }.expect("invalid bootinfo memory map")
}

unsafe fn ident_map_bootinfo(
    mapper: &mut EarlyMapper<'_>,
    bootinfo_paddr: PhysAddr,
) -> View<'static> {
    let pfn = bootinfo_paddr.containing_frame();
    let vpn = VirtPageNum::new(pfn.as_usize());

    let header = bootinfo_paddr.as_usize() as *const ItemHeader;

    let mut pointer = MappingPointer::new(vpn, 1);
    mapper
        .map(&mut pointer, pfn, PageTablePerms::READ)
        .expect("failed to map initial bootinfo page");

    let view = unsafe { View::new(header) }.expect("invalid bootinfo");
    let view_pages = align_up(view.total_size(), PAGE_SIZE);

    pointer = MappingPointer::new(vpn, view_pages);
    pointer.advance(1); // Skip first mapped page
    mapper
        .map(&mut pointer, pfn, PageTablePerms::READ)
        .expect("failed to map full bootinfo");

    view
}

unsafe fn ident_unmap_bootinfo(
    mapper: &mut EarlyMapper<'_>,
    bootinfo_paddr: PhysAddr,
    view_size: usize,
) {
    let vpn = VirtPageNum::new(bootinfo_paddr.containing_frame().as_usize());
    let pages = align_up(view_size, PAGE_SIZE);

    mapper
        .unmap(&mut MappingPointer::new(vpn, pages), &mut NoopGather)
        .expect("failed to unmap early bootinfo");

    // TODO: TLB flush
}
