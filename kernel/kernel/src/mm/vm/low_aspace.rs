use core::slice;

use alloc::rc::Rc;

use crate::arch::mm::{LOW_ASPACE_BASE, LOW_ASPACE_END};
use crate::arch::mmu::{flush_low_tlb, flush_low_tlb_page, prepare_low_pt_root, PT_ENTRY_COUNT};
use crate::err::Result;
use crate::mm::physmap::pfn_to_physmap;
use crate::mm::pmm::FrameBox;
use crate::mm::pt::clear_page_table;
use crate::mm::types::{AccessMode, PageTablePerms, PhysFrameNum};

use super::aspace::{AddrSpace, AddrSpaceOps, TlbFlush};

pub struct LowAddrSpaceOps {
    root_pt: FrameBox,
    allowed_access_mode: AccessMode,
}

pub type LowAddrSpace = AddrSpace<LowAddrSpaceOps>;

pub fn make_low_addr_space(allowed_access_mode: AccessMode) -> Result<Rc<LowAddrSpace>> {
    let root_pt = make_root_pt()?;

    // Safety: we have a brand-new page table and complete control of entries in the low half of the
    // address space.
    let aspace = unsafe {
        LowAddrSpace::new(
            LOW_ASPACE_BASE..LOW_ASPACE_END,
            LowAddrSpaceOps {
                root_pt,
                allowed_access_mode,
            },
        )?
    };
    let aspace = Rc::try_new(aspace)?;

    Ok(aspace)
}

fn make_root_pt() -> Result<FrameBox> {
    let root_pt = FrameBox::new()?;
    let root_pt_addr = pfn_to_physmap(root_pt.pfn()).addr();

    // Safety: we own the page table as we have just allocated it, and it contains sufficient space.
    unsafe {
        clear_page_table(root_pt_addr.as_mut_ptr());
        let root_pt_slice = slice::from_raw_parts_mut(root_pt_addr.as_mut_ptr(), PT_ENTRY_COUNT);
        prepare_low_pt_root(root_pt_slice);
    }

    Ok(root_pt)
}

unsafe impl AddrSpaceOps for LowAddrSpaceOps {
    fn root_pt(&self) -> PhysFrameNum {
        self.root_pt.pfn()
    }

    fn flush(&self, request: TlbFlush<'_>) {
        // TODO: shootdown here
        match request {
            TlbFlush::Specific(pages) => {
                for &vpn in pages {
                    flush_low_tlb_page(vpn);
                }
            }
            TlbFlush::All => flush_low_tlb(),
        }
    }

    fn can_cull_pt(&self, _pt: PhysFrameNum, _level: usize) -> bool {
        // We don't own any tables of our own, they are all allocated dynamically by the address
        // space as necessary.
        true
    }

    fn base_perms(&self) -> PageTablePerms {
        match self.allowed_access_mode {
            AccessMode::User => PageTablePerms::USER,
            AccessMode::Kernel => PageTablePerms::empty(),
        }
    }
}
