use core::alloc::Layout;
use core::ops::Range;

use arrayvec::ArrayVec;

use crate::arch::mmu::{flush_tlb, PageTableSpace};
use crate::{arch, kimage};

use super::pt::{
    GatherInvalidations, MappingPointer, PageTable, PageTableAlloc, PageTableAllocError,
    TranslatePhys,
};
use super::types::{PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

const EARLY_MAP_MAX_SLOTS: usize = 5;
const EARLY_MAP_PT_PAGES: usize = 10;

static EARLY_MAP_PTS: [PageTableSpace; EARLY_MAP_PT_PAGES] =
    [PageTableSpace::NEW; EARLY_MAP_PT_PAGES];

pub struct BootHeap {
    base: PhysAddr,
    cur: PhysAddr,
    end: PhysAddr,
}

impl BootHeap {
    pub fn new(range: Range<PhysAddr>) -> Self {
        Self {
            base: range.start,
            cur: range.start,
            end: range.end,
        }
    }

    pub fn used_range(&self) -> Range<PhysAddr> {
        self.base..self.cur
    }

    pub fn alloc_phys(&mut self, layout: Layout) -> PhysAddr {
        let base = self.cur.align_up(layout.align());
        if base > self.end || layout.size() > self.end - base {
            panic!("bootheap exhausted");
        }

        self.cur = base + layout.size();
        base
    }
}

impl PageTableAlloc for BootHeap {
    fn allocate(&mut self) -> Result<PhysFrameNum, PageTableAllocError> {
        Ok(self
            .alloc_phys(Layout::new::<PageTableSpace>())
            .containing_frame())
    }
}

pub struct EarlyMapPfnTranslator(Range<PhysFrameNum>);

impl EarlyMapPfnTranslator {
    pub fn new(phys_range: Range<PhysFrameNum>) -> Self {
        Self(phys_range)
    }
}

impl TranslatePhys for EarlyMapPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        if kimage::contains_phys(phys) {
            return kimage::vpn_from_kernel_pfn(phys);
        }

        assert!(
            self.0.contains(&phys),
            "page not covered by early identity map"
        );
        VirtPageNum::new(phys.as_usize())
    }
}

/// # Safety
///
/// * This function must be called only once during initialization on the BSP
/// * The returned object must be accessed only on the BSP
pub unsafe fn get_early_mapper() -> EarlyMapper {
    let addr = VirtAddr::from_ptr(EARLY_MAP_PTS.as_ptr());
    let start = kimage::pfn_from_kernel_vpn(addr.containing_page());
    let alloc = BumpPageTableAlloc {
        cur: start,
        end: start + EARLY_MAP_PTS.len(),
    };

    let pt = unsafe { PageTable::new(arch::mmu::kernel_pt_root(), KernelPfnTranslator) };

    EarlyMapper {
        slots: ArrayVec::new(),
        pt,
        alloc,
    }
}

pub struct EarlyMapper {
    slots: ArrayVec<EarlyMapperSlot, EARLY_MAP_MAX_SLOTS>,
    pt: PageTable<KernelPfnTranslator>,
    alloc: BumpPageTableAlloc,
}

impl EarlyMapper {
    pub fn map(&mut self, base: PhysFrameNum, pages: usize) -> VirtPageNum {
        let virt = VirtPageNum::new(base.as_usize());

        // Safety: our allocator allocates directly out of the kernel image, and we are guaranteed
        // not to reuse existing allocations by the safety contract of `get_early_mapper`.
        unsafe {
            self.pt
                .map(
                    &mut self.alloc,
                    &mut MappingPointer::new(virt, pages),
                    base,
                    PageTablePerms::READ | PageTablePerms::WRITE,
                )
                .expect("early map failed");
        }

        virt
    }
}

impl Drop for EarlyMapper {
    fn drop(&mut self) {
        for slot in &self.slots {
            // Safety: we should have exclusive access to the page tables at this point
            // (single-core), our callers need unsafe to access the mapped pages anyway.
            unsafe {
                self.pt
                    .unmap(
                        &mut self.alloc,
                        &mut NoopGather,
                        &mut MappingPointer::new(
                            VirtPageNum::new(slot.base.as_usize()),
                            slot.pages,
                        ),
                    )
                    .expect("early unmap failed");
            }
        }

        flush_tlb();
    }
}

struct EarlyMapperSlot {
    base: PhysFrameNum,
    pages: usize,
}

struct BumpPageTableAlloc {
    cur: PhysFrameNum,
    end: PhysFrameNum,
}

impl PageTableAlloc for BumpPageTableAlloc {
    fn allocate(&mut self) -> Result<PhysFrameNum, PageTableAllocError> {
        if self.cur >= self.end {
            return Err(PageTableAllocError);
        }

        let ret = self.cur;
        self.cur += 1;

        Ok(ret)
    }
}

struct NoopGather;

impl GatherInvalidations for NoopGather {
    fn add_tlb_flush(&mut self, _vpn: VirtPageNum) {}
}

struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        kimage::vpn_from_kernel_pfn(phys)
    }
}
