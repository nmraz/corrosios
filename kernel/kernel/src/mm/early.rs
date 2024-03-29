use core::alloc::Layout;
use core::marker::PhantomData;
use core::ops::Range;

use arrayvec::ArrayVec;
use spin_once::TakeOnce;

use crate::arch::mm::EARLY_MAP_PT_PAGES;
use crate::arch::mmu::{flush_kernel_tlb, kernel_pt_root, PageTableSpace};
use crate::err::{Error, Result};
use crate::kimage;

use super::pt::{MappingPointer, NoopGather, PageTable, PageTableAlloc, TranslatePhys};
use super::types::{CacheMode, PageTablePerms, PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

const EARLY_MAP_MAX_SLOTS: usize = 5;

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

    pub fn range(&self) -> Range<PhysAddr> {
        self.base..self.end
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
    fn allocate(&mut self) -> Result<PhysFrameNum> {
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

/// Returns an `EarlyMapper` object that can be used to identity-map regions of memory before the
/// physmap is set up.
///
/// # Panics
///
/// Panics if this function is called more than once.
pub fn take_early_mapper() -> EarlyMapper {
    static GUARD: TakeOnce<()> = TakeOnce::new();

    GUARD.take_init(()).expect("early mapper already taken");

    let addr = VirtAddr::from_ptr(EARLY_MAP_PTS.as_ptr());
    let start = kimage::pfn_from_kernel_vpn(addr.containing_page());
    let alloc = BumpPageTableAlloc {
        cur: start,
        end: start + EARLY_MAP_PTS.len(),
    };

    let pt = unsafe { PageTable::new(kernel_pt_root(), KernelPfnTranslator) };

    EarlyMapper {
        slots: ArrayVec::new(),
        pt,
        alloc,
        _not_send_sync: PhantomData,
    }
}

pub struct EarlyMapper {
    slots: ArrayVec<EarlyMapperSlot, EARLY_MAP_MAX_SLOTS>,
    pt: PageTable<KernelPfnTranslator>,
    alloc: BumpPageTableAlloc,
    _not_send_sync: PhantomData<*const ()>,
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
                    CacheMode::Cached,
                )
                .expect("early map failed");
        }

        self.slots.push(EarlyMapperSlot { base, pages });

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
                        &mut NoopGather,
                        &mut MappingPointer::new(
                            VirtPageNum::new(slot.base.as_usize()),
                            slot.pages,
                        ),
                    )
                    .expect("early unmap failed");
            }
        }

        flush_kernel_tlb();
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
    fn allocate(&mut self) -> Result<PhysFrameNum> {
        if self.cur >= self.end {
            return Err(Error::OUT_OF_MEMORY);
        }

        let ret = self.cur;
        self.cur += 1;

        Ok(ret)
    }
}

struct KernelPfnTranslator;

impl TranslatePhys for KernelPfnTranslator {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum {
        kimage::vpn_from_kernel_pfn(phys)
    }
}
