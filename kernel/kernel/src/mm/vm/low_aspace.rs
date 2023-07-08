use core::{ptr, slice};

use alloc::sync::Arc;

use crate::arch::mm::{LOW_ASPACE_BASE, LOW_ASPACE_END};
use crate::arch::mmu::{
    flush_low_tlb, flush_low_tlb_page, prepare_low_pt_root, set_low_root_pt, PT_ENTRY_COUNT,
};
use crate::err::Result;
use crate::mm::physmap::pfn_to_physmap;
use crate::mm::pmm::FrameBox;
use crate::mm::pt::clear_page_table;
use crate::mm::types::{AccessMode, PageTablePerms, PhysFrameNum};
use crate::sync::resched::ReschedDisabled;

use super::aspace::{AddrSpace, AddrSpaceOps, TlbFlush};

pub struct LowAddrSpaceOps {
    root_pt: FrameBox,
    allowed_access_mode: AccessMode,
}

pub type LowAddrSpace = AddrSpace<LowAddrSpaceOps>;

pub fn make_low_addr_space(allowed_access_mode: AccessMode) -> Result<Arc<LowAddrSpace>> {
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
    let aspace = Arc::try_new(aspace)?;

    Ok(aspace)
}

/// Switches the current low address space from `old_aspace` to `new_aspace`, performing any
/// necessary flushes and architectural state updates.
///
/// This is a very low-level function that should generally not be called directly (except by
/// context-switching code).
///
/// If `new_aspace` is `None`, the current low address space will be unmapped entirely, leaving only
/// the high kernel address space mapped.
///
/// # Safety
///
/// Callers must guarantee that `old_aspace` is the one that was currently active on the current
/// CPU.
///
/// Beyond the fixed requirements, this function is wildly unsafe, as it replaces the entire
/// lower-half address space with a different one. The caller must ensure that all accesses to low
/// memory are made in accordance with the new address space after the switch.
pub unsafe fn switch_to(
    _resched_disabled: &ReschedDisabled,
    old_aspace: Option<&LowAddrSpace>,
    new_aspace: Option<&LowAddrSpace>,
) {
    if raw_aspace_ptr(new_aspace) == raw_aspace_ptr(old_aspace) {
        // Address space is already active, nothing to update/flush.
        return;
    }

    let new_pt = new_aspace.map(|aspace| aspace.ops().root_pt());
    unsafe {
        set_low_root_pt(new_pt);
    }
}

fn raw_aspace_ptr(aspace: Option<&LowAddrSpace>) -> *const LowAddrSpace {
    aspace.map_or(ptr::null(), |p| p)
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
