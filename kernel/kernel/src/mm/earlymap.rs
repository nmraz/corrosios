use crate::arch::mmu::PageTableSpace;
use crate::kimage;

use super::pt::{GatherInvalidations, Mapper, PageTableAlloc, PageTableAllocError, TranslatePhys};
use super::types::{PhysFrameNum, VirtAddr, VirtPageNum};

pub type EarlyMapper<'a> = Mapper<'a, BumpPageTableAlloc, KernelPfnTranslator>;

/// # Safety
///
/// The provided root table must be correctly structured, and all referenced/allocated page tables
/// must lie in the kernel image.
pub unsafe fn make_early_mapper(
    root_pt: PhysFrameNum,
    alloc: &mut BumpPageTableAlloc,
) -> EarlyMapper<'_> {
    // Safety: function contract
    unsafe { EarlyMapper::new(root_pt, alloc, KernelPfnTranslator) }
}

pub struct BumpPageTableAlloc {
    cur: PhysFrameNum,
    end: PhysFrameNum,
}

impl BumpPageTableAlloc {
    pub fn from_kernel_space(space: &'static [PageTableSpace]) -> Self {
        let addr = VirtAddr::from_ptr(space.as_ptr());

        let start = kimage::pfn_from_kernel_vpn(addr.containing_page());
        let pages = space.len();

        Self {
            cur: start,
            end: start + pages,
        }
    }
}

unsafe impl PageTableAlloc for BumpPageTableAlloc {
    fn allocate(&mut self) -> Result<PhysFrameNum, PageTableAllocError> {
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
    fn add_pt_dealloc(&mut self, _pt: PhysFrameNum) {}
}

pub struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        kimage::vpn_from_kernel_pfn(phys)
    }
}
