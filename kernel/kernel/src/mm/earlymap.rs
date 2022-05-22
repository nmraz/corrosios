use crate::arch::kernel_vmspace::KERNEL_IMAGE_SPACE_BASE;
use crate::arch::mmu::PageTable;

use super::pt::{GatherInvalidations, Mapper, PageTableAlloc, PageTableAllocError, TranslatePhys};
use super::types::{PhysPageNum, VirtAddr, VirtPageNum};

pub type EarlyMapper<'a> = Mapper<'a, BumpPageTableAlloc, KernelPfnTranslator>;

/// # Safety
///
/// The provided root table must be correctly structured, and all referenced/allocated page tables
/// must lie in the kernel image.
pub unsafe fn make_early_mapper<'a>(
    root_pt: &'a PageTable,
    alloc: &'a mut BumpPageTableAlloc,
) -> EarlyMapper<'a> {
    // Safety: function contract
    unsafe { EarlyMapper::new(root_pt, alloc, KernelPfnTranslator) }
}

pub struct BumpPageTableAlloc {
    cur: PhysPageNum,
    end: PhysPageNum,
}

impl BumpPageTableAlloc {
    pub fn from_kernel_space(space: &'static [PageTable]) -> Self {
        let addr = VirtAddr::from_ptr(space.as_ptr());

        let start = pfn_from_kernel_vaddr(addr);
        let pages = space.len();

        Self {
            cur: start,
            end: start + pages,
        }
    }
}

unsafe impl PageTableAlloc for BumpPageTableAlloc {
    fn allocate(&mut self) -> Result<PhysPageNum, PageTableAllocError> {
        if self.cur >= self.end {
            return Err(PageTableAllocError);
        }

        let ret = self.cur;
        self.cur += 1;

        Ok(ret)
    }
}

pub struct NoopGather;

impl GatherInvalidations for NoopGather {
    fn add_tlb_flush(&mut self, _vpn: VirtPageNum) {}
    fn add_pt_dealloc(&mut self, _pt: PhysPageNum) {}
}

pub struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysPageNum) -> VirtPageNum {
        vpn_from_kernel_pfn(phys)
    }
}

fn pfn_from_kernel_vaddr(vaddr: VirtAddr) -> PhysPageNum {
    PhysPageNum::new(vaddr.containing_page() - KERNEL_IMAGE_SPACE_BASE)
}

fn vpn_from_kernel_pfn(pfn: PhysPageNum) -> VirtPageNum {
    KERNEL_IMAGE_SPACE_BASE + pfn.as_usize()
}
