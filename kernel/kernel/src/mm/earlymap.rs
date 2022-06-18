use crate::arch::mmu::PageTableSpace;
use crate::{arch, kimage};

use super::pt::{
    GatherInvalidations, PageTable, PageTableAlloc, PageTableAllocError, TranslatePhys,
};
use super::types::{PhysFrameNum, VirtAddr, VirtPageNum};

pub type EarlyPageTable = PageTable<KernelPfnTranslator>;

/// # Safety
///
/// All page tables referenced by the kernel root page table must lie in the kernel image for the
/// duration of this object's lifetime.
pub unsafe fn get_early_page_table() -> EarlyPageTable {
    // Safety: function contract
    unsafe { EarlyPageTable::new(arch::mmu::kernel_pt_root(), KernelPfnTranslator) }
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
}

pub struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        kimage::vpn_from_kernel_pfn(phys)
    }
}
