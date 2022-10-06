use core::cmp;
use core::ops::Range;

use alloc::boxed::Box;
use alloc::sync::Arc;
use arrayvec::ArrayString;
use intrusive_collections::rbtree::CursorMut;
use intrusive_collections::{intrusive_adapter, Bound, KeyAdapter, RBTree, RBTreeLink};
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
    pub unsafe fn new(range: Range<VirtPageNum>, ops: O) -> Result<Arc<Self>> {
        assert!(range.end >= range.start);

        let inner = AddrSpaceInner {
            cell_owner: QCellOwner::new(),
        };

        let root_slice = Arc::try_new(AddrSpaceSliceShared {
            name: ArrayString::from("root").unwrap(),
            base: range.start,
            page_count: range.end - range.start,
            inner: inner.cell_owner.cell(Some(AddrSpaceSliceInner::new())),
        })?;

        Ok(Arc::try_new(AddrSpace {
            inner: SpinLock::new(inner),
            root_slice,
            ops,
        })?)
    }

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
        base: Option<VirtPageNum>,
        page_count: usize,
    ) -> Result<AddrSpaceSlice<O>> {
        let name = ArrayString::from(&name[..cmp::min(name.len(), MAX_NAME_LEN)]).unwrap();

        let slice = self.with_inner(|inner, id| {
            inner.alloc_spot(base, page_count, |base| {
                let slice = Arc::try_new(AddrSpaceSliceShared {
                    name,
                    base,
                    page_count,
                    inner: QCell::new(id, Some(AddrSpaceSliceInner::new())),
                })?;

                let child = AddrSpaceChild::Subslice(Arc::clone(&slice));
                Ok((child, slice))
            })
        })?;

        Ok(AddrSpaceSlice {
            addr_space: Arc::clone(&self.addr_space),
            slice,
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
    children: RBTree<AddrSpaceChildAdapter<O>>,
}

impl<O> AddrSpaceSliceInner<O> {
    fn new() -> Self {
        Self {
            children: RBTree::default(),
        }
    }

    fn alloc_spot<R>(
        &mut self,
        base: Option<VirtPageNum>,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        match base {
            Some(base) => self.alloc_spot_fixed(base, page_count, || f(base)),
            None => self.alloc_spot_dynamic(page_count, f),
        }
    }

    fn alloc_spot_dynamic<R>(
        &mut self,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        todo!()
    }

    fn alloc_spot_fixed<R>(
        &mut self,
        base: VirtPageNum,
        page_count: usize,
        f: impl FnOnce() -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        let prev = self.children.upper_bound_mut(Bound::Included(&base));
        if let Some(prev) = prev.get() {
            if prev.base() + prev.page_count() > base {
                return Err(Error::RESOURCE_IN_USE);
            }
        }

        if let Some(next) = prev.peek_next().get() {
            if base + page_count > next.base() {
                return Err(Error::RESOURCE_IN_USE);
            }
        }

        finish_insert(prev, f)
    }
}

fn finish_insert<O, R>(
    mut prev: CursorMut<'_, AddrSpaceChildAdapter<O>>,
    f: impl FnOnce() -> Result<(AddrSpaceChild<O>, R)>,
) -> Result<R> {
    let new_child = Box::try_new_uninit()?;
    let (data, ret) = f()?;
    let new_child = Box::write(
        new_child,
        AddrSpaceChildNode {
            link: RBTreeLink::new(),
            data,
        },
    );
    prev.insert_after(new_child);
    Ok(ret)
}

struct AddrSpaceChildNode<O> {
    link: RBTreeLink,
    data: AddrSpaceChild<O>,
}

impl<O> AddrSpaceChildNode<O> {
    fn base(&self) -> VirtPageNum {
        match &self.data {
            AddrSpaceChild::Subslice(slice) => slice.base,
            AddrSpaceChild::Mapping(mapping) => mapping.base,
        }
    }

    fn page_count(&self) -> usize {
        match &self.data {
            AddrSpaceChild::Subslice(slice) => slice.page_count,
            AddrSpaceChild::Mapping(mapping) => mapping.page_count,
        }
    }
}

intrusive_adapter!(AddrSpaceChildAdapter<O> = Box<AddrSpaceChildNode<O>>: AddrSpaceChildNode<O> { link: RBTreeLink });
impl<'a, O> KeyAdapter<'a> for AddrSpaceChildAdapter<O> {
    type Key = VirtPageNum;

    fn get_key(&self, value: &'a AddrSpaceChildNode<O>) -> Self::Key {
        value.base()
    }
}

enum AddrSpaceChild<O> {
    Subslice(Arc<AddrSpaceSliceShared<O>>),
    Mapping(Mapping),
}

struct Mapping {
    base: VirtPageNum,
    page_count: usize,
}
