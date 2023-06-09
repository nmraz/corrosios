use core::cell::RefCell;
use core::slice;

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
use crate::mp::current_percpu;
use crate::sync::resched::ReschedDisabled;

use super::aspace::{AddrSpace, AddrSpaceOps, TlbFlush};

pub struct LowAddrSpaceOps {
    root_pt: FrameBox,
    allowed_access_mode: AccessMode,
}

pub type LowAddrSpace = AddrSpace<LowAddrSpaceOps>;

pub(super) struct Context {
    current_aspace: RefCell<Option<Arc<LowAddrSpace>>>,
}

impl Context {
    /// Creates a new per-cpu address space context.
    ///
    /// Note: this function may be called very early during initialization (before anything is set
    /// up), so it must not allocate or take any locks.
    pub fn new() -> Self {
        Self {
            current_aspace: RefCell::new(None),
        }
    }
}

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

/// Returns a new owning reference to the current active address space.
pub fn current(resched_disabled: &ReschedDisabled) -> Option<Arc<LowAddrSpace>> {
    with_current(resched_disabled, |current| current.cloned())
}

/// Invokes `f` on the current low address space, returning its return value.
///
/// Note that `f` must not call [`switch_to`]; doing so will panic at runtime.
pub fn with_current<R>(
    resched_disabled: &ReschedDisabled,
    f: impl FnOnce(Option<&Arc<LowAddrSpace>>) -> R,
) -> R {
    f(current_aspace(resched_disabled).borrow().as_ref())
}

/// Temporarily enters `aspace` and invokes `f`, then restores the original active address space.
///
/// # Safety
///
/// This function is wildly unsafe, as it replaces the entire lower-half address space with a
/// different one. The caller must ensure that all accesses to low memory made by `f` are made in
/// accordance with `aspace`.
pub unsafe fn enter_with<R>(
    resched_disabled: &ReschedDisabled,
    aspace: Arc<LowAddrSpace>,
    f: impl FnOnce() -> R,
) -> R {
    let orig_aspace = current(resched_disabled);
    unsafe {
        switch_to(resched_disabled, Some(aspace));
    }
    let ret = f();
    unsafe {
        switch_to(resched_disabled, orig_aspace);
    }
    ret
}

/// Switches the current low address space to `aspace`, performing any necessary flushes and
/// architectural state updates.
///
/// If `aspace` is `None`, the current low address space will be unmapped entirely, leaving only
/// the high kernel address space mapped.
///
/// # Panics
///
/// This function will panic if called from within a call to [`with_current`] on the current CPU.
///
/// # Safety
///
/// This function is wildly unsafe, as it replaces the entire lower-half address space with a
/// different one. The caller must ensure that all accesses to low memory are made in accordance
/// with the new address space after the switch.
pub unsafe fn switch_to(resched_disabled: &ReschedDisabled, aspace: Option<Arc<LowAddrSpace>>) {
    let mut active_aspace = current_aspace(resched_disabled).borrow_mut();
    if raw_aspace_ptr(&aspace) == raw_aspace_ptr(&active_aspace) {
        // Address space is already active, nothing to update/flush.
        return;
    }

    let new_pt = aspace.as_ref().map(|aspace| aspace.ops().root_pt());
    unsafe {
        set_low_root_pt(new_pt);
    }

    *active_aspace = aspace;
}

fn raw_aspace_ptr(aspace: &Option<Arc<LowAddrSpace>>) -> *const LowAddrSpace {
    aspace.as_ref().map_or(core::ptr::null(), Arc::as_ptr)
}

fn current_aspace(resched_disabled: &ReschedDisabled) -> &RefCell<Option<Arc<LowAddrSpace>>> {
    &current_percpu(resched_disabled)
        .vm
        .aspace_context
        .current_aspace
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
