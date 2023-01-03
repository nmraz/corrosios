use core::ops::ControlFlow;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use object_name::Name;
use qcell::{QCell, QCellOwner, QCellOwnerID};

use crate::err::{Error, Result};
use crate::mm::types::{Protection, VirtPageNum};
use crate::mm::vm::object::VmObject;

/// A child of an address space slice, containing either another slice or a mapping.
pub enum SliceChild {
    Subslice(Arc<Slice>),
    Mapping(Arc<Mapping>),
}

impl SliceChild {
    fn start(&self) -> VirtPageNum {
        match self {
            SliceChild::Subslice(subslice) => subslice.start(),
            SliceChild::Mapping(mapping) => mapping.start(),
        }
    }

    fn end(&self) -> VirtPageNum {
        match self {
            SliceChild::Subslice(subslice) => subslice.end(),
            SliceChild::Mapping(mapping) => mapping.end(),
        }
    }
}

/// Represents a slice of an address space.
pub struct Slice {
    name: Name,
    start: VirtPageNum,
    page_count: usize,
    inner: QCell<Option<SliceInner>>,
}

impl Slice {
    pub fn new(
        owner: QCellOwnerID,
        parent: Option<Arc<Slice>>,
        name: &str,
        start: VirtPageNum,
        page_count: usize,
    ) -> Result<Arc<Self>> {
        let slice = Arc::try_new(Slice {
            name: Name::new(name),
            start,
            page_count,
            inner: QCell::new(
                owner,
                Some(SliceInner {
                    parent,
                    children: BTreeMap::new(),
                }),
            ),
        })?;

        Ok(slice)
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn start(&self) -> VirtPageNum {
        self.start
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    pub fn end(&self) -> VirtPageNum {
        self.start + self.page_count
    }

    pub fn parent(&self, owner: &QCellOwner) -> Result<Option<Arc<Slice>>> {
        Ok(self.inner(owner)?.parent.as_ref().cloned())
    }

    /// Retrieves the mapping containing `vpn`, recursing into subslices as necessary.
    pub fn get_mapping<'a>(
        &'a self,
        owner: &'a QCellOwner,
        vpn: VirtPageNum,
    ) -> Result<&'a Mapping> {
        self.check_vpn(vpn)?;

        let mut slice = self;

        loop {
            let inner = slice.inner(owner)?;
            let child = inner.get_child(vpn).ok_or(Error::BAD_ADDRESS)?;
            match child {
                SliceChild::Subslice(subslice) => slice = subslice,
                SliceChild::Mapping(mapping) => return Ok(mapping),
            }
        }
    }

    /// Allocates a child of size `page_count` from within this slice, invoking `f` to construct it
    /// once a suitable area has been found.
    ///
    /// If `start` is provided, the child will be created at the requested virtual page number.
    /// Otherwise, a sufficiently large available region will be found and used.
    pub fn alloc_spot<R>(
        &self,
        owner: &mut QCellOwner,
        start: Option<VirtPageNum>,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(SliceChild, R)>,
    ) -> Result<R> {
        match start {
            Some(start) => self.alloc_spot_fixed(owner, start, page_count, || f(start)),
            None => self.alloc_spot_dynamic(owner, page_count, f),
        }
    }

    /// Removes the direct child of `self` based at `start`.
    ///
    /// # Panics
    ///
    /// Panics if `self` does not have a child starting at `start`.
    pub fn remove_child(&self, owner: &mut QCellOwner, start: VirtPageNum) -> Result<()> {
        self.inner_mut(owner)?
            .children
            .remove(&start)
            .expect("no child for provided start address");
        Ok(())
    }

    /// Recursively detaches all subslices and of `self`.
    ///
    /// When this operation completes, `self` will be in the detached state.
    ///
    /// # Panics
    ///
    /// Panics if `self` is already detached.
    pub fn detach_children(self: &Arc<Self>, owner: &mut QCellOwner) {
        let mut cur = Arc::clone(self);

        loop {
            let first_child = cur
                .inner_mut(owner)
                .expect("current slice should still be attached")
                .children
                .pop_first();

            if let Some((_, child)) = first_child {
                match child {
                    SliceChild::Subslice(subslice) => {
                        cur = subslice;
                    }
                    SliceChild::Mapping(_) => {}
                }
            } else {
                // Now that we've finished detaching children, mark the current slice as detached
                // and move back up to the parent if necessary.
                let inner = cur
                    .inner
                    .rw(owner)
                    .take()
                    .expect("current slice should still be attached");

                if !Arc::ptr_eq(&cur, self) {
                    // We're in a (nested) child, move back up to the parent.
                    cur = inner.parent.expect("child slice should have a parent");
                } else {
                    break;
                }
            }
        }
    }

    /// Allocates a child of size `page_count` from within this slice, invoking `f` to construct it
    /// once a suitable area has been found.
    fn alloc_spot_dynamic<R>(
        &self,
        owner: &mut QCellOwner,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(SliceChild, R)>,
    ) -> Result<R> {
        let gap_start = self
            .iter_gaps_mut(owner, |gap_start, gap_page_count| {
                if gap_page_count > page_count {
                    ControlFlow::Break(gap_start)
                } else {
                    ControlFlow::Continue(())
                }
            })?
            .ok_or(Error::OUT_OF_RESOURCES)?;

        let (child, retval) = f(gap_start)?;
        self.inner_mut(owner)
            .expect("slice should still be attached")
            .children
            .insert(gap_start, child);

        Ok(retval)
    }

    /// Allocates a child spanning `start..start + page_count` from within this slice, invoking `f`
    /// to construct it once a suitable area has been found.
    fn alloc_spot_fixed<R>(
        &self,
        owner: &mut QCellOwner,
        start: VirtPageNum,
        page_count: usize,
        f: impl FnOnce() -> Result<(SliceChild, R)>,
    ) -> Result<R> {
        let end = start
            .checked_add(page_count)
            .ok_or(Error::INVALID_ARGUMENT)?;

        if start < self.start || end > self.end() {
            return Err(Error::INVALID_ARGUMENT);
        }

        let inner = self.inner_mut(owner)?;

        if let Some((_, prev)) = inner.children.range(..start).next_back() {
            if prev.end() > start {
                return Err(Error::RESOURCE_OVERLAP);
            }
        }

        if let Some((_, next)) = inner.children.range(start..).next() {
            if end > next.start() {
                return Err(Error::RESOURCE_OVERLAP);
            }
        }

        let (child, ret) = f()?;
        inner.children.insert(start, child);
        Ok(ret)
    }

    /// Calls `f` on all gaps (unallocated regions) in this slice, passing each invocation the start
    /// of the gap and its page count.
    ///
    /// Iteration will stop early if `f` returns [`ControlFlow::Break`], and the break value will
    /// be returned.
    fn iter_gaps_mut<'a, B>(
        &'a self,
        owner: &'a QCellOwner,
        mut f: impl FnMut(VirtPageNum, usize) -> ControlFlow<B>,
    ) -> Result<Option<B>> {
        let inner = self.inner(owner)?;

        let mut iter = inner.children.iter();

        let Some((&first_start, first)) = iter.next() else {
            let retval = match f(self.start, self.page_count) {
                ControlFlow::Break(val) => Some(val),
                ControlFlow::Continue(_) => None,
            };

            return Ok(retval);
        };

        if self.start < first_start {
            if let ControlFlow::Break(val) = f(self.start, first_start - self.start) {
                return Ok(Some(val));
            }
        }

        let mut last_end = first.end();

        for (&cur_start, cur) in iter {
            if last_end < cur_start {
                if let ControlFlow::Break(val) = f(last_end, cur_start - last_end) {
                    return Ok(Some(val));
                }
            }
            last_end = cur.end();
        }

        if last_end < self.end() {
            if let ControlFlow::Break(val) = f(last_end, self.end() - last_end) {
                return Ok(Some(val));
            }
        }

        Ok(None)
    }

    /// Checks that `vpn` lies within this slice's range, returning `BAD_ADDRESS` if it does not.
    fn check_vpn(&self, vpn: VirtPageNum) -> Result<()> {
        if (self.start..self.end()).contains(&vpn) {
            Ok(())
        } else {
            Err(Error::BAD_ADDRESS)
        }
    }

    fn inner<'a>(&'a self, owner: &'a QCellOwner) -> Result<&'a SliceInner> {
        self.inner.ro(owner).as_ref().ok_or(Error::INVALID_STATE)
    }

    fn inner_mut<'a>(&'a self, owner: &'a mut QCellOwner) -> Result<&'a mut SliceInner> {
        self.inner.rw(owner).as_mut().ok_or(Error::INVALID_STATE)
    }
}

/// Represents a mapping of a VM object in an address space.
pub struct Mapping {
    start: VirtPageNum,
    page_count: usize,
    object_offset: usize,
    object: Arc<dyn VmObject>,
    inner: QCell<Option<MappingInner>>,
}

impl Mapping {
    pub fn new(
        owner: QCellOwnerID,
        parent: Arc<Slice>,
        start: VirtPageNum,
        page_count: usize,
        object: Arc<dyn VmObject>,
        object_offset: usize,
        prot: Protection,
    ) -> Result<Arc<Self>> {
        let mapping = Arc::try_new(Mapping {
            start,
            page_count,
            object_offset,
            object,
            inner: QCell::new(owner, Some(MappingInner { parent, prot })),
        })?;
        Ok(mapping)
    }

    pub fn start(&self) -> VirtPageNum {
        self.start
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    pub fn end(&self) -> VirtPageNum {
        self.start + self.page_count
    }

    pub fn object_offset(&self) -> usize {
        self.object_offset
    }

    pub fn object(&self) -> &Arc<dyn VmObject> {
        &self.object
    }

    pub fn parent(&self, owner: &QCellOwner) -> Result<Arc<Slice>> {
        Ok(Arc::clone(&self.inner(owner)?.parent))
    }

    pub fn prot(&self, owner: &QCellOwner) -> Result<Protection> {
        self.inner(owner).map(|inner| inner.prot)
    }

    fn inner<'a>(&'a self, owner: &'a QCellOwner) -> Result<&'a MappingInner> {
        self.inner.ro(owner).as_ref().ok_or(Error::INVALID_STATE)
    }
}

struct SliceInner {
    // This apparent cycle is broken by calls to `detach_children`, which guarantee that this whole
    // inner structure is destroyed when appropriate.
    parent: Option<Arc<Slice>>,
    children: BTreeMap<VirtPageNum, SliceChild>,
}

impl SliceInner {
    /// Retrives the direct child of `self` containing `vpn`, if one exists.
    fn get_child(&self, vpn: VirtPageNum) -> Option<&SliceChild> {
        self.children
            .range(..vpn)
            .next_back()
            .filter(|(_, child)| vpn < child.end())
            .map(|(_, child)| child)
    }
}

struct MappingInner {
    // This apparent cycle is broken by calls to `detach_children`, which guarantee that this whole
    // inner structure is destroyed when appropriate.
    parent: Arc<Slice>,
    prot: Protection,
}
