use bootinfo::item::{MemoryKind, MemoryRange};
use bootinfo::view::View;
use bootinfo::{ItemHeader, ItemKind};

use crate::arch::kernel_vmspace::{PHYS_MAP_BASE, PHYS_MAP_PT_PAGES};
use crate::arch::mmu::{PageTable, PAGE_SIZE};

use super::earlymap::{self, BumpPageTableAlloc, EarlyMapper, NoopGather};
use super::pt::MappingPointer;
use super::types::{PageTablePerms, PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};

const BOOTINFO_PT_PAGES: usize = 10;

static mut PHYS_MAP_PT_SPACE: [PageTable; PHYS_MAP_PT_PAGES + BOOTINFO_PT_PAGES] =
    [PageTable::new(); PHYS_MAP_PT_PAGES + BOOTINFO_PT_PAGES];

/// # Safety
///
/// * This function must be called only once on the bootstrap processor
/// * There should be no live references to the kernel root page table
/// * The kernel root page table should refer only to page tables allocated from within the kernel
/// image
pub unsafe fn init(bootinfo_paddr: PhysAddr) {
    // Safety: function contract
    let mut alloc = unsafe { BumpPageTableAlloc::from_kernel_space(&mut PHYS_MAP_PT_SPACE) };
    let kernel_pt = unsafe { &mut *crate::arch::mmu::kernel_pt_root() };

    let mut mapper = unsafe { earlymap::make_early_mapper(kernel_pt, &mut alloc) };

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

pub fn ppn_to_physmap(ppn: PhysPageNum) -> VirtPageNum {
    PHYS_MAP_BASE + ppn.as_usize()
}

fn init_inner(mapper: &mut EarlyMapper<'_>, bootinfo: View<'_>) {
    let mem_map = get_mem_map(bootinfo);
    let usable_map = mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE);

    for range in usable_map {
        let phys = PhysPageNum::new(range.start_page);
        let virt = ppn_to_physmap(phys);

        println!(
            "usable range {:#x}-{:#x}",
            phys.as_usize(),
            phys.as_usize() + range.page_count
        );

        let mut pointer = MappingPointer::new(virt, range.page_count);
        mapper
            .map(
                &mut pointer,
                phys,
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
    let ppn = bootinfo_paddr.containing_page();
    let vpn = VirtPageNum::new(ppn.as_usize());

    let header = bootinfo_paddr.as_usize() as *const ItemHeader;

    let mut pointer = MappingPointer::new(vpn, 1);
    mapper
        .map(&mut pointer, ppn, PageTablePerms::READ)
        .expect("failed to map initial bootinfo page");

    let view = unsafe { View::new(&*header) }.expect("invalid bootinfo");
    let view_pages = required_pages(view.total_size());

    pointer = MappingPointer::new(vpn, view_pages);
    pointer.advance(1); // Skip first mapped page
    mapper
        .map(&mut pointer, ppn, PageTablePerms::READ)
        .expect("failed to map full bootinfo");

    view
}

unsafe fn ident_unmap_bootinfo(
    mapper: &mut EarlyMapper<'_>,
    bootinfo_paddr: PhysAddr,
    view_size: usize,
) {
    let vpn = VirtPageNum::new(bootinfo_paddr.containing_page().as_usize());
    let pages = required_pages(view_size);

    mapper
        .unmap(&mut MappingPointer::new(vpn, pages), &mut NoopGather)
        .expect("failed to unmap early bootinfo");

    // TODO: TLB flush
}

fn required_pages(size: usize) -> usize {
    (size + PAGE_SIZE - 1) / PAGE_SIZE
}
