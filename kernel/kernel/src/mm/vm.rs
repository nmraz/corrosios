use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use qcell::{QCell, QCellOwner};

use crate::err::{Error, Result};
use crate::sync::irq::IrqDisabled;
use crate::sync::SpinLock;

use super::types::{PhysFrameNum, VirtPageNum};

/// A request to flush pages from the TLB.
pub enum TlbFlush<'a> {
    /// Flush only the specified pages from the TLB.
    Specific(&'a [VirtPageNum]),
    /// FLush the entire TLB.
    All,
}

/// Encapsulates the necessary low-level page table interactions required for higher-level address
/// spaces.
///
/// # Safety
///
/// Implementors must ensure that [`root_pt`](AddrSpaceOps::root_pt) returns a valid frame
/// usable as a page table.
pub unsafe trait AddrSpaceOps {
    /// Requests the root page table. All accesses to this table will be synchronized by the
    /// address space lock.
    fn root_pt(&self) -> PhysFrameNum;

    /// Called when the address space becomes active on a specific CPU.
    ///
    /// **Note:** this function should not switch the hardware page table, that will be handled by
    /// the [`AddrSpace`] itself.
    fn enter(&self, irq_disabled: &IrqDisabled);

    /// Called when the address space is switched away from on a specific CPU.
    fn exit(&self, irq_disabled: &IrqDisabled);

    /// Requests a TLB flush.
    ///
    /// This function should block until the request completes.
    fn flush(&self, request: &TlbFlush<'_>);
}

pub struct AddrSpace<O> {
    lock: SpinLock<AddrSpaceInner>,
    root_slice: Arc<AddrSpaceSliceShared<O>>,
    ops: O,
}

impl<O: AddrSpaceOps> AddrSpace<O> {
    pub fn root_slice(&self) -> AddrSpaceSlice<O> {
        let slice = Arc::downgrade(&self.root_slice);
        AddrSpaceSlice { slice }
    }
}

#[derive(Clone)]
pub struct AddrSpaceSlice<O> {
    slice: Weak<AddrSpaceSliceShared<O>>,
}

impl<O> AddrSpaceSlice<O> {
    pub fn addr_space(&self) -> Result<Arc<AddrSpace<O>>> {
        self.upgrade().map(|slice| slice.addr_space())
    }

    pub fn create_subslice(
        &self,
        base_offset: Option<usize>,
        page_count: usize,
    ) -> Result<AddrSpaceSlice<O>> {
        let shared = self.upgrade()?;

        todo!()
    }

    fn upgrade(&self) -> Result<Arc<AddrSpaceSliceShared<O>>> {
        self.slice.upgrade().ok_or(Error::INVALID_STATE)
    }
}

struct AddrSpaceSliceShared<O> {
    base: VirtPageNum,
    page_count: usize,
    addr_space: Weak<AddrSpace<O>>,
    inner: QCell<AddrSpaceSliceInner<O>>,
}

impl<O> AddrSpaceSliceShared<O> {
    fn addr_space(&self) -> Arc<AddrSpace<O>> {
        self.addr_space
            .upgrade()
            .expect("slice data outlived address space")
    }
}

struct AddrSpaceInner {
    cell_owner: QCellOwner,
}

struct AddrSpaceSliceInner<O> {
    children: BTreeMap<VirtPageNum, AddrSpaceSliceChild<O>>,
}

enum AddrSpaceSliceChild<O> {
    Subslice(Arc<AddrSpaceSlice<O>>),
    Mapping(Box<Mapping>),
}

struct Mapping {
    base: VirtPageNum,
    page_count: usize,
}
