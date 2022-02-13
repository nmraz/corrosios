use core::ptr;

use bootinfo::view::View;
use bootinfo::ItemHeader;

use crate::arch::kernel_vmspace::{BOOTINFO_SPACE_BASE, KERNEL_IMAGE_SPACE_BASE};
use crate::arch::mmu::{PageTable, PAGE_SIZE};
use crate::mm::pt::Mapper;

use super::pt::{PageTableAlloc, PageTableAllocError, TranslatePhys};
use super::types::{PageTablePerms, PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};

/// # Safety
///
/// This function must be called only once on the bootstrap processor; there should be no live
/// references to the kernel root page table.
pub unsafe fn map_bootinfo(bootinfo_paddr: PhysAddr) -> View<'static> {
    let bootinfo_pt_pfn =
        pfn_from_kernel_vaddr(VirtAddr::from_ptr(ptr::addr_of!(BOOTINFO_PT_SPACE)));

    // Safety: function contract
    let kernel_pt = unsafe { &mut *crate::arch::mmu::kernel_pt_root() };
    let mut pt_alloc = BumpPageTableAlloc::new(bootinfo_pt_pfn, BOOTINFO_PT_PAGES);

    // Safety: all page tables lie within the kernel image mapping, allowing us to use `KernelPfnTranslator`.
    let mut mapper = unsafe { Mapper::new(kernel_pt, &mut pt_alloc, KernelPfnTranslator) };

    let perms = PageTablePerms::empty();

    let bootinfo_pfn = bootinfo_paddr.containing_page();
    mapper
        .map(BOOTINFO_SPACE_BASE, bootinfo_pfn, perms)
        .expect("failed to map initial bootinfo page");

    let bootinfo_ptr: *const ItemHeader = BOOTINFO_SPACE_BASE.addr().as_ptr();
    let view = unsafe { View::new(&*bootinfo_ptr) }.expect("invalid bootinfo");

    let bootinfo_pages = (view.total_size() + PAGE_SIZE - 1) / PAGE_SIZE;
    for page in 1..bootinfo_pages {
        let vpn = VirtPageNum::new(BOOTINFO_SPACE_BASE.as_usize() + page);
        let pfn = PhysPageNum::new(bootinfo_paddr.as_usize() + page);

        mapper
            .map(vpn, pfn, perms)
            .expect("failed to map bootinfo page");
    }

    view
}

const BOOTINFO_PT_PAGES: usize = 10;
static mut BOOTINFO_PT_SPACE: [PageTable; BOOTINFO_PT_PAGES] =
    [PageTable::new(); BOOTINFO_PT_PAGES];

struct BumpPageTableAlloc {
    cur: PhysPageNum,
    end: PhysPageNum,
}

impl BumpPageTableAlloc {
    pub fn new(start: PhysPageNum, pages: usize) -> Self {
        Self {
            cur: start,
            end: PhysPageNum::new(start.as_usize() + pages),
        }
    }
}

unsafe impl PageTableAlloc for BumpPageTableAlloc {
    fn allocate(&mut self) -> Result<PhysPageNum, PageTableAllocError> {
        if self.cur.as_usize() >= self.end.as_usize() {
            return Err(PageTableAllocError);
        }

        let ret = self.cur;
        self.cur = PhysPageNum::new(ret.as_usize() + 1);

        Ok(ret)
    }

    unsafe fn deallocate(&mut self, _pfn: PhysPageNum) {}
}

struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysPageNum) -> VirtPageNum {
        vpn_from_kernel_pfn(phys)
    }
}

fn pfn_from_kernel_vaddr(vaddr: VirtAddr) -> PhysPageNum {
    PhysPageNum::new(vaddr.containing_page().as_usize() - KERNEL_IMAGE_SPACE_BASE.as_usize())
}

fn vpn_from_kernel_pfn(pfn: PhysPageNum) -> VirtPageNum {
    VirtPageNum::new(pfn.as_usize() + KERNEL_IMAGE_SPACE_BASE.as_usize())
}
