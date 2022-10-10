use core::cmp;
use core::ops::{ControlFlow, Range};

use alloc::boxed::Box;
use alloc::sync::Arc;
use arrayvec::ArrayString;
use intrusive_collections::rbtree::CursorMut;
use intrusive_collections::{intrusive_adapter, Bound, KeyAdapter, RBTree, RBTreeLink};
use qcell::{QCell, QCellOwner};

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
            start: range.start,
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

    fn with_owner<R>(&self, f: impl FnOnce(&mut QCellOwner) -> Result<R>) -> Result<R> {
        self.inner.with(|inner, _| f(&mut inner.cell_owner))
    }
}

#[derive(Clone)]
pub struct AddrSpaceSlice<O> {
    addr_space: Arc<AddrSpace<O>>,
    slice: Arc<AddrSpaceSliceShared<O>>,
}

impl<O: AddrSpaceOps> AddrSpaceSlice<O> {
    pub fn name(&self) -> &str {
        &self.slice.name
    }

    pub fn start(&self) -> VirtPageNum {
        self.slice.start
    }

    pub fn end(&self) -> VirtPageNum {
        self.start() + self.page_count()
    }

    pub fn page_count(&self) -> usize {
        self.slice.page_count
    }

    pub fn create_subslice(
        &self,
        name: &str,
        start: Option<VirtPageNum>,
        page_count: usize,
    ) -> Result<AddrSpaceSlice<O>> {
        let name = ArrayString::from(&name[..cmp::min(name.len(), MAX_NAME_LEN)]).unwrap();

        let slice = self.addr_space.with_owner(|owner| {
            let id = owner.id();

            self.slice.alloc_spot(owner, start, page_count, |start| {
                let slice = Arc::try_new(AddrSpaceSliceShared {
                    name,
                    start,
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
}

struct AddrSpaceSliceShared<O> {
    name: ArrayString<32>,
    start: VirtPageNum,
    page_count: usize,
    inner: QCell<Option<AddrSpaceSliceInner<O>>>,
}

impl<O> AddrSpaceSliceShared<O> {
    fn alloc_spot<R>(
        &self,
        owner: &mut QCellOwner,
        start: Option<VirtPageNum>,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        match start {
            Some(start) => self.alloc_spot_fixed(owner, start, page_count, || f(start)),
            None => self.alloc_spot_dynamic(owner, page_count, f),
        }
    }

    fn alloc_spot_dynamic<R>(
        &self,
        owner: &mut QCellOwner,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        let mut f = Some(f);

        self.iter_gaps_mut(owner, |gap_start, gap_page_count, prev_cursor| {
            if gap_page_count > page_count {
                let f = f.take().expect("did not break after finding spot");
                ControlFlow::Break(finish_insert_after(prev_cursor, || f(gap_start)))
            } else {
                ControlFlow::Continue(())
            }
        })
        .and_then(|res| res.unwrap_or(Err(Error::OUT_OF_MEMORY)))
    }

    fn alloc_spot_fixed<R>(
        &self,
        owner: &mut QCellOwner,
        start: VirtPageNum,
        page_count: usize,
        f: impl FnOnce() -> Result<(AddrSpaceChild<O>, R)>,
    ) -> Result<R> {
        let end = start
            .checked_add(page_count)
            .ok_or(Error::INVALID_ARGUMENT)?;

        if start < self.start || end > self.start + self.page_count {
            return Err(Error::INVALID_ARGUMENT);
        }

        let inner = self.inner(owner)?;

        let mut prev = inner.children.upper_bound_mut(Bound::Included(&start));
        if let Some(prev) = prev.get() {
            if prev.start() + prev.page_count() > start {
                return Err(Error::RESOURCE_IN_USE);
            }
        }

        if let Some(next) = prev.peek_next().get() {
            if end > next.start() {
                return Err(Error::RESOURCE_IN_USE);
            }
        }

        finish_insert_after(&mut prev, f)
    }

    /// Calls `f` on all gaps (unallocated regions) in this slice, passing each invocation the start
    /// of the gap, its page count, and a cursor pointing to the item in the tree before the gap.
    ///
    /// Iteration will stop early if `f` returns [`ControlFlow::Break`], and the break value will
    /// be returned.
    fn iter_gaps_mut<'a, B>(
        &'a self,
        owner: &'a mut QCellOwner,
        mut f: impl FnMut(
            VirtPageNum,
            usize,
            &mut CursorMut<'a, AddrSpaceChildAdapter<O>>,
        ) -> ControlFlow<B>,
    ) -> Result<Option<B>> {
        let inner = self.inner(owner)?;

        let mut cursor = inner.children.front_mut();
        let Some(first) = cursor.get() else {
            let retval = match f(self.start, self.page_count, &mut cursor) {
                ControlFlow::Break(val) => Some(val),
                ControlFlow::Continue(_) => None,
            };

            return Ok(retval);
        };

        let first_start = first.start();

        if self.start < first_start {
            if let ControlFlow::Break(val) = f(self.start, first_start - self.start, &mut cursor) {
                return Ok(Some(val));
            }
        }

        while let Some(next) = cursor.peek_next().get() {
            let cur = cursor
                .get()
                .expect("cursor null despite next being non-null");

            let cur_end = cur.end();
            let next_start = next.start();

            if cur_end < next_start {
                if let ControlFlow::Break(val) = f(cur_end, next_start - cur_end, &mut cursor) {
                    return Ok(Some(val));
                }
            }

            cursor.move_next();
        }

        let last_end = cursor
            .get()
            .expect("cursor should point to last node")
            .end();
        let slice_end = self.start + self.page_count;
        if last_end < slice_end {
            if let ControlFlow::Break(val) = f(last_end, slice_end - last_end, &mut cursor) {
                return Ok(Some(val));
            }
        }

        Ok(None)
    }

    fn inner<'a>(&'a self, owner: &'a mut QCellOwner) -> Result<&'a mut AddrSpaceSliceInner<O>> {
        self.inner.rw(owner).as_mut().ok_or(Error::INVALID_STATE)
    }
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
}

fn finish_insert_after<O, R>(
    prev: &mut CursorMut<'_, AddrSpaceChildAdapter<O>>,
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
    fn start(&self) -> VirtPageNum {
        match &self.data {
            AddrSpaceChild::Subslice(slice) => slice.start,
            AddrSpaceChild::Mapping(mapping) => mapping.start,
        }
    }

    fn end(&self) -> VirtPageNum {
        self.start() + self.page_count()
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
        value.start()
    }
}

enum AddrSpaceChild<O> {
    Subslice(Arc<AddrSpaceSliceShared<O>>),
    Mapping(Mapping),
}

struct Mapping {
    start: VirtPageNum,
    page_count: usize,
}
