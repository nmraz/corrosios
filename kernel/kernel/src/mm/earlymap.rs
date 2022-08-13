use arrayvec::ArrayVec;

use crate::arch::mmu::{flush_tlb, PageTableSpace};
use crate::{arch, kimage};

use super::pt::{
    GatherInvalidations, MappingPointer, PageTable, PageTableAlloc, PageTableAllocError,
    TranslatePhys,
};
use super::types::{PageTablePerms, PhysFrameNum, VirtAddr, VirtPageNum};

pub type EarlyPageTable = PageTable<KernelPfnTranslator>;

/// # Safety
///
/// All page tables referenced by the kernel root page table must lie in the kernel image for the
/// duration of this object's lifetime.
pub unsafe fn get_early_page_table() -> EarlyPageTable {
    // Safety: function contract
    unsafe { EarlyPageTable::new(arch::mmu::kernel_pt_root(), KernelPfnTranslator) }
}

pub unsafe fn get_early_mapper() -> EarlyMapper {
    let addr = VirtAddr::from_ptr(EARLY_MAP_PTS.as_ptr());
    let start = kimage::pfn_from_kernel_vpn(addr.containing_page());
    let alloc = BumpPageTableAlloc {
        cur: start,
        end: start + EARLY_MAP_PTS.len(),
    };

    let pt = unsafe { EarlyPageTable::new(arch::mmu::kernel_pt_root(), KernelPfnTranslator) };

    EarlyMapper {
        slots: ArrayVec::new(),
        pt,
        alloc,
    }
}

const EARLY_MAP_MAX_SLOTS: usize = 5;
const EARLY_MAP_PT_PAGES: usize = 10;

static EARLY_MAP_PTS: [PageTableSpace; EARLY_MAP_PT_PAGES] =
    [PageTableSpace::NEW; EARLY_MAP_PT_PAGES];

pub struct EarlyMapper {
    slots: ArrayVec<EarlyMapperSlot, EARLY_MAP_MAX_SLOTS>,
    pt: PageTable<KernelPfnTranslator>,
    alloc: BumpPageTableAlloc,
}

impl EarlyMapper {
    pub fn map(&mut self, base: PhysFrameNum, pages: usize) {
        self.pt
            .map(
                &mut self.alloc,
                &mut MappingPointer::new(VirtPageNum::new(base.as_usize()), pages),
                base,
                PageTablePerms::READ | PageTablePerms::WRITE,
            )
            .expect("early map failed");
    }
}

impl Drop for EarlyMapper {
    fn drop(&mut self) {
        for slot in &self.slots {
            self.pt
                .unmap(
                    &mut self.alloc,
                    &mut NoopGather,
                    &mut MappingPointer::new(VirtPageNum::new(slot.base.as_usize()), slot.pages),
                )
                .expect("early unmap failed");
        }

        flush_tlb();
    }
}

struct EarlyMapperSlot {
    base: PhysFrameNum,
    pages: usize,
}

pub struct BumpPageTableAlloc {
    cur: PhysFrameNum,
    end: PhysFrameNum,
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
