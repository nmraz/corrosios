use core::cmp;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use arrayvec::ArrayString;
use qcell::{QCell, QCellOwner, QCellOwnerID};

use crate::err::{Error, Result};
use crate::sync::irq::IrqDisabled;
use crate::sync::SpinLock;

use super::types::{PhysFrameNum, VirtPageNum};

const MAX_NAME_LEN: usize = 32;

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
    inner: SpinLock<AddrSpaceInner>,
    root_slice: Arc<AddrSpaceSliceShared<O>>,
    ops: O,
}

impl<O: AddrSpaceOps> AddrSpace<O> {
    pub fn root_slice(self: &Arc<Self>) -> AddrSpaceSlice<O> {
        AddrSpaceSlice {
            slice: Arc::clone(&self.root_slice),
            addr_space: Arc::clone(self),
        }
    }
}

#[derive(Clone)]
pub struct AddrSpaceSlice<O> {
    addr_space: Arc<AddrSpace<O>>,
    slice: Arc<AddrSpaceSliceShared<O>>,
}

impl<O> AddrSpaceSlice<O> {
    pub fn name(&self) -> &str {
        &self.slice.name
    }

    pub fn base(&self) -> VirtPageNum {
        self.slice.base
    }

    pub fn page_count(&self) -> usize {
        self.slice.page_count
    }

    pub fn create_subslice(
        &self,
        name: &str,
        base_offset: Option<usize>,
        page_count: usize,
    ) -> Result<AddrSpaceSlice<O>> {
        let name = ArrayString::from(&name[..cmp::min(name.len(), MAX_NAME_LEN)]).unwrap();

        self.with_inner(|inner, id| {
            let slice = Arc::try_new(AddrSpaceSliceShared {
                name,
                base: self.base(), // TODO
                page_count,
                inner: QCell::new(id, Some(AddrSpaceSliceInner::new())),
            })
            .map_err(|_| Error::OUT_OF_MEMORY)?;

            Ok(AddrSpaceSlice {
                addr_space: Arc::clone(&self.addr_space),
                slice,
            })
        })
    }

    fn with_inner<R>(
        &self,
        f: impl FnOnce(&mut AddrSpaceSliceInner<O>, QCellOwnerID) -> Result<R>,
    ) -> Result<R> {
        self.addr_space.inner.with(|addr_space_inner, _| {
            let id = addr_space_inner.cell_owner.id();
            let inner = self
                .slice
                .inner
                .rw(&mut addr_space_inner.cell_owner)
                .as_mut()
                .ok_or(Error::INVALID_STATE)?;
            f(inner, id)
        })
    }
}

struct AddrSpaceSliceShared<O> {
    name: ArrayString<32>,
    base: VirtPageNum,
    page_count: usize,
    inner: QCell<Option<AddrSpaceSliceInner<O>>>,
}

struct AddrSpaceInner {
    cell_owner: QCellOwner,
}

struct AddrSpaceSliceInner<O> {
    children: BTreeMap<VirtPageNum, AddrSpaceSliceChild<O>>,
}

impl<O> AddrSpaceSliceInner<O> {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
        }
    }
}

enum AddrSpaceSliceChild<O> {
    Subslice(Arc<AddrSpaceSlice<O>>),
    Mapping(Box<Mapping>),
}

struct Mapping {
    base: VirtPageNum,
    page_count: usize,
}
