use core::cmp;
use core::ops::{ControlFlow, Range};

use alloc::boxed::Box;
use alloc::sync::Arc;
use arrayvec::ArrayString;
use intrusive_collections::rbtree::CursorMut;
use intrusive_collections::{intrusive_adapter, Bound, KeyAdapter, RBTree, RBTreeAtomicLink};
use qcell::{QCell, QCellOwner};

use crate::err::{Error, Result};
use crate::mm::physmap::PhysmapPfnTranslator;
use crate::mm::pmm::PmmPageTableAlloc;
use crate::mm::pt::{MappingPointer, PageTable};
use crate::mm::types::{PageTablePerms, PhysFrameNum, VirtPageNum};
use crate::sync::SpinLock;

use super::object::VmObject;
use super::AccessType;

const MAX_NAME_LEN: usize = 32;

/// A request to flush pages from the TLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Requests a TLB flush.
    ///
    /// This function should block until the request completes.
    fn flush(&self, request: &TlbFlush<'_>);
}

/// Represents an address space, with its associated page tables and mappings.
///
/// An instance of this structure is the entry point for all high-level virtual memory operations,
/// such as mapping in pages and handling page faults.
///
/// # Slices
///
/// Unlike most typical address space abstractions, these address spaces do not provide generic
/// `map` and `unmap` operations taking an arbitrary virtual address ranges. Instead, every address
/// space is exposed to users as a tree of disjoint ["slices"](SliceHandle), each covering a
/// portion of the address space.
///
/// Every slice can contain more sub-slices and (leaf) mapping objects pointing to a [`VmObject`].
/// Mapping and unmapping operations operate on a given slice handle, and can only modify its direct
/// children. The root slice of an address space can be retrieved via
/// [`root_slice`](AddrSpace::root_slice).
///
/// Beyond providing encapsulation, slices also make reservation of virtual address ranges explicit
/// and make it easier to
///
/// # Page tables and synchronization
///
/// Access to the low-level page table is abstracted via the [`AddrSpaceOps`] trait, which is
/// responsible for providing access to the root page table for this address space and maintaining
/// consistency across processors.
pub struct AddrSpace<O> {
    inner: SpinLock<AddrSpaceInner>,
    root_slice: Arc<SliceData>,
    ops: O,
}

struct AddrSpaceInner {
    cell_owner: QCellOwner,
}

impl<O: AddrSpaceOps> AddrSpace<O> {
    /// Creates a new address space spanning `range`, with page table operations `ops`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ops` can be used to manipulate any mappings in the range
    /// `range`.
    pub unsafe fn new(range: Range<VirtPageNum>, ops: O) -> Result<Self> {
        assert!(range.end >= range.start);

        let inner = AddrSpaceInner {
            cell_owner: QCellOwner::new(),
        };

        let root_slice = Arc::try_new(SliceData {
            name: ArrayString::from("root").unwrap(),
            start: range.start,
            page_count: range.end - range.start,
            inner: inner.cell_owner.cell(Some(SliceInner::new())),
        })?;

        Ok(AddrSpace {
            inner: SpinLock::new(inner),
            root_slice,
            ops,
        })
    }

    /// Returns the underlying page table operations.
    pub fn ops(&self) -> &O {
        &self.ops
    }

    /// Retrieves a handle to the root slice of this address space.
    pub fn root_slice(&self) -> SliceHandle {
        SliceHandle {
            slice: Arc::clone(&self.root_slice),
        }
    }

    /// Handles a page fault accessing `vpn` with access type `access_type`.
    ///
    /// This may ultimately call into [`provide_page`](VmObject::provide_page) on the object mapped
    /// at the specified address.
    ///
    /// # Errors
    ///
    /// * `BAD_ADDRESS` - `vpn` is not mapped into this address space.
    /// * `NO_PERMS` - `vpn` is mapped with permissions incompatible with `access_type`.
    /// * Any errors returned by the underlying `provide_page` call.
    pub fn fault(&self, vpn: VirtPageNum, access_type: AccessType) -> Result<()> {
        struct GetCommitRangeByVpn(VirtPageNum);
        impl<'a> GetCommitRange<'a> for GetCommitRangeByVpn {
            fn get_range<'b, O>(
                &self,
                addr_space: &'a AddrSpace<O>,
                owner: &'b QCellOwner,
            ) -> Result<CommitRange<'b>>
            where
                'a: 'b,
            {
                let mapping = addr_space.root_slice.get_mapping(owner, self.0)?;
                let offset = self.0 - mapping.start;
                Ok(CommitRange {
                    mapping,
                    offset,
                    page_count: 1,
                })
            }
        }

        self.do_commit(access_type, GetCommitRangeByVpn(vpn))
    }

    /// Allocates a sub-slice spanning `page_count` pages from within `slice`.
    ///
    /// A human-friendly description of this slice's purpose should be passed in `name`; it will be
    /// used only for debugging purposes and may be truncated.
    ///
    /// If `start` is provided, the subslice will be created at the requested virtual page number.
    /// Otherwise, a sufficiently large available region will be found and used.
    ///
    /// # Errors
    ///
    /// * `INVALID_STATE` - This function was called with a [detached](SliceHandle#states) slice.
    /// * `INVALID_ARGUMENT` - The requested range is too large or does not lie in the virtual
    ///                        address range managed by this slice.
    /// * `OUT_OF_MEMORY` - Allocation of the new metadata failed.
    /// * `RESOURCE_OVERLAP` - The requested range overlaps an existing subslice or mapping.
    /// * `OUT_OF_RESOURCES` - No available regions of the requested size were found.
    ///
    /// # Panics
    ///
    /// Panics if `slice` belongs to a different address space.
    pub fn create_subslice(
        &self,
        slice: &SliceHandle,
        name: &str,
        start: Option<VirtPageNum>,
        page_count: usize,
    ) -> Result<SliceHandle> {
        let name = ArrayString::from(&name[..cmp::min(name.len(), MAX_NAME_LEN)]).unwrap();

        let slice = self.with_owner(|owner| {
            let id = owner.id();

            slice.slice.alloc_spot(owner, start, page_count, |start| {
                let slice = Arc::try_new(SliceData {
                    name,
                    start,
                    page_count,
                    inner: QCell::new(id, Some(SliceInner::new())),
                })?;

                let child = AddrSpaceChild::Subslice(Arc::clone(&slice));
                Ok((child, slice))
            })
        })?;

        Ok(SliceHandle { slice })
    }

    /// Maps the range `object_offset..object_offset + page_count` of `object` into `slice`.
    ///
    /// The mapping will be created with the permissions specified in `perms`.
    ///
    /// If `start` is provided, the mapping will be created at the requested virtual page number.
    /// Otherwise, a sufficiently large available region will be found and used.
    ///
    /// # Errors
    ///
    /// * `INVALID_STATE` - This function was called on a [detached](SliceHandle#states) slice.
    /// * `INVALID_ARGUMENT` - The requested range is too large or does not lie in the virtual
    ///                        address range managed by this slice, or `page_count` is larger
    ///                        than the size of the object.
    /// * `OUT_OF_MEMORY` - Allocation of the new metadata failed.
    /// * `RESOURCE_OVERLAP` - The requested range overlaps an existing subslice or mapping.
    /// * `OUT_OF_RESOURCES` - No available regions of the requested size were found.
    ///
    /// # Panics
    ///
    /// Panics if `slice` belongs to a different address space.
    pub fn map(
        &self,
        slice: &SliceHandle,
        start: Option<VirtPageNum>,
        page_count: usize,
        object_offset: usize,
        object: Arc<dyn VmObject>,
        perms: PageTablePerms,
    ) -> Result<MappingHandle> {
        let mapping = self.with_owner(|owner| {
            let id = owner.id();
            slice.slice.alloc_spot(owner, start, page_count, |start| {
                let mapping = Arc::try_new(MappingData {
                    start,
                    page_count,
                    object_offset,
                    object,
                    inner: QCell::new(id, Some(MappingInner::new(perms))),
                })?;

                let child = AddrSpaceChild::Mapping(Arc::clone(&mapping));
                Ok((child, mapping))
            })
        })?;

        Ok(MappingHandle { mapping })
    }

    pub fn commit(
        &self,
        mapping: &MappingHandle,
        access_type: AccessType,
        offset: usize,
        page_count: usize,
    ) -> Result<()> {
        struct GetReadyCommitRange<'a>(CommitRange<'a>);
        impl<'a> GetCommitRange<'a> for GetReadyCommitRange<'a> {
            fn get_range<'b, O>(
                &self,
                _addr_space: &'a AddrSpace<O>,
                _owner: &'b QCellOwner,
            ) -> Result<CommitRange<'b>>
            where
                'a: 'b,
            {
                Ok(self.0)
            }
        }

        let commit_range = CommitRange {
            mapping: &mapping.mapping,
            offset,
            page_count,
        };
        self.do_commit(access_type, GetReadyCommitRange(commit_range))
    }

    fn do_commit<'a>(&'a self, access_type: AccessType, g: impl GetCommitRange<'a>) -> Result<()> {
        // TODO: be more careful about this lock when `provide_page` can sleep.
        self.with_owner(|owner| {
            let range = g.get_range(self, owner)?;
            let mapping = range.mapping;
            let perms = mapping
                .inner
                .ro(owner)
                .as_ref()
                .ok_or(Error::INVALID_STATE)?
                .perms;

            if !access_allowed(access_type, perms) {
                return Err(Error::NO_PERMS);
            }

            // TODO: refactor this and find some way for `provide_page` to block outside the
            // critical section
            for offset in range.offset..range.offset + range.page_count {
                let object_offset = offset + mapping.object_offset;

                let pfn = mapping.object.provide_page(object_offset, access_type)?;

                // Safety: we're holding the page table lock, and our translator and allocator perform
                // correctly.
                unsafe {
                    let mut pt = PageTable::new(self.ops.root_pt(), PhysmapPfnTranslator);
                    pt.map(
                        &mut PmmPageTableAlloc,
                        &mut MappingPointer::new(mapping.start + range.offset, 1),
                        pfn,
                        perms,
                    )?;
                };
            }

            Ok(())
        })
    }

    fn with_owner<R>(&self, f: impl FnOnce(&mut QCellOwner) -> Result<R>) -> Result<R> {
        self.inner.with(|inner, _| f(&mut inner.cell_owner))
    }
}

#[derive(Clone, Copy)]
struct CommitRange<'a> {
    mapping: &'a MappingData,
    offset: usize,
    page_count: usize,
}

trait GetCommitRange<'a> {
    fn get_range<'b, O>(
        &self,
        addr_space: &'a AddrSpace<O>,
        owner: &'b QCellOwner,
    ) -> Result<CommitRange<'b>>
    where
        'a: 'b;
}

fn access_allowed(access_type: AccessType, perms: PageTablePerms) -> bool {
    match access_type {
        AccessType::Read => perms.contains(PageTablePerms::READ),
        AccessType::Write => perms.contains(PageTablePerms::WRITE),
        AccessType::Execute => perms.contains(PageTablePerms::EXECUTE),
    }
}

/// A handle to a [slice](AddrSpace#slices) of an address space.
///
/// # States
///
/// In general, every slice may be either *attached* or *detached*.
///
/// Every slice is created attached (and the root of an address space is always attached),
/// but unmapping a slice from its parent detaches it. Any attempts to perform mapping-related
/// operations on a detached slice will fail with [`INVALID_STATE`](crate::err::Error::INVALID_STATE).
#[derive(Clone)]
pub struct SliceHandle {
    slice: Arc<SliceData>,
}

impl SliceHandle {
    /// Returns the human-friendly name of this slice, useful for debugging purposes.
    ///
    /// The root slice of an address space is always named `root`.
    pub fn name(&self) -> &str {
        &self.slice.name
    }

    /// Returns the first page number covered by this slice.
    pub fn start(&self) -> VirtPageNum {
        self.slice.start
    }

    /// Returns the page number just after the last page covered by this slice.
    pub fn end(&self) -> VirtPageNum {
        self.start() + self.page_count()
    }

    /// Returns the number of pages covered by this slice.
    pub fn page_count(&self) -> usize {
        self.slice.page_count
    }
}

/// A handle to a mapping of a VM object into an address space.
///
/// # States
///
/// Like slices, every mapping may be either *attached* or *detached*.
///
/// Every mapping is created attached, but unmapping a mapping from its parent detaches it. Any
/// attempts to perform mapping-related operations on a detached mapping will fail with
/// [`INVALID_STATE`](crate::err::Error::INVALID_STATE).
#[derive(Clone)]
pub struct MappingHandle {
    mapping: Arc<MappingData>,
}

impl MappingHandle {
    /// Returns the first page number covered by this mapping.
    pub fn start(&self) -> VirtPageNum {
        self.mapping.start
    }

    /// Returns the page number just after the last page covered by this mapping.
    pub fn end(&self) -> VirtPageNum {
        self.start() + self.page_count()
    }

    /// Returns the number of pages covered by this mapping.
    pub fn page_count(&self) -> usize {
        self.mapping.page_count
    }

    /// Returns the offset in the VM object at which this mapping starts.
    pub fn object_offset(&self) -> usize {
        self.mapping.object_offset
    }

    /// Returns a handle to the underlying VM object.
    pub fn object(&self) -> &Arc<dyn VmObject> {
        &self.mapping.object
    }
}

struct SliceData {
    name: ArrayString<32>,
    start: VirtPageNum,
    page_count: usize,
    inner: QCell<Option<SliceInner>>,
}

impl SliceData {
    /// Retrieves the mapping containing `vpn`, recursing into subslices as necessary.
    fn get_mapping<'a>(
        &'a self,
        owner: &'a QCellOwner,
        vpn: VirtPageNum,
    ) -> Result<&'a MappingData> {
        self.check_vpn(vpn)?;

        let inner = self.inner(owner)?;
        let child = inner.get_child(vpn).ok_or(Error::BAD_ADDRESS)?;

        match child {
            AddrSpaceChild::Subslice(slice) => slice.get_mapping(owner, vpn),
            AddrSpaceChild::Mapping(mapping) => Ok(mapping),
        }
    }

    /// Allocates a child of size `page_count` from within this slice, invoking `f` to construct it
    /// once a suitable area has been found.
    ///
    /// If `start` is provided, the child will be created at the requested virtual page number.
    /// Otherwise, a sufficiently large available region will be found and used.
    fn alloc_spot<R>(
        &self,
        owner: &mut QCellOwner,
        start: Option<VirtPageNum>,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild, R)>,
    ) -> Result<R> {
        match start {
            Some(start) => self.alloc_spot_fixed(owner, start, page_count, || f(start)),
            None => self.alloc_spot_dynamic(owner, page_count, f),
        }
    }

    /// Allocates a child of size `page_count` from within this slice, invoking `f` to construct it
    /// once a suitable area has been found.
    fn alloc_spot_dynamic<R>(
        &self,
        owner: &mut QCellOwner,
        page_count: usize,
        f: impl FnOnce(VirtPageNum) -> Result<(AddrSpaceChild, R)>,
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
        .and_then(|res| res.unwrap_or(Err(Error::OUT_OF_RESOURCES)))
    }

    /// Allocates a child spanning `start..start + page_count` from within this slice, invoking `f`
    /// to construct it once a suitable area has been found.
    fn alloc_spot_fixed<R>(
        &self,
        owner: &mut QCellOwner,
        start: VirtPageNum,
        page_count: usize,
        f: impl FnOnce() -> Result<(AddrSpaceChild, R)>,
    ) -> Result<R> {
        let end = start
            .checked_add(page_count)
            .ok_or(Error::INVALID_ARGUMENT)?;

        if start < self.start || end > self.end() {
            return Err(Error::INVALID_ARGUMENT);
        }

        let inner = self.inner_mut(owner)?;

        let mut prev = inner.children.upper_bound_mut(Bound::Included(&start));
        if let Some(prev) = prev.get() {
            if prev.end() > start {
                return Err(Error::RESOURCE_OVERLAP);
            }
        }

        if let Some(next) = prev.peek_next().get() {
            if end > next.start() {
                return Err(Error::RESOURCE_OVERLAP);
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
            &mut CursorMut<'a, AddrSpaceChildAdapter>,
        ) -> ControlFlow<B>,
    ) -> Result<Option<B>> {
        let inner = self.inner_mut(owner)?;

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
        let slice_end = self.end();
        if last_end < slice_end {
            if let ControlFlow::Break(val) = f(last_end, slice_end - last_end, &mut cursor) {
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

    fn end(&self) -> VirtPageNum {
        self.start + self.page_count
    }

    fn inner<'a>(&'a self, owner: &'a QCellOwner) -> Result<&'a SliceInner> {
        self.inner.ro(owner).as_ref().ok_or(Error::INVALID_STATE)
    }

    fn inner_mut<'a>(&'a self, owner: &'a mut QCellOwner) -> Result<&'a mut SliceInner> {
        self.inner.rw(owner).as_mut().ok_or(Error::INVALID_STATE)
    }
}

fn finish_insert_after<R>(
    prev: &mut CursorMut<'_, AddrSpaceChildAdapter>,
    f: impl FnOnce() -> Result<(AddrSpaceChild, R)>,
) -> Result<R> {
    let new_child = Box::try_new_uninit()?;
    let (data, ret) = f()?;
    let new_child = Box::write(
        new_child,
        AddrSpaceChildNode {
            link: RBTreeAtomicLink::new(),
            data,
        },
    );
    prev.insert_after(new_child);
    Ok(ret)
}

struct SliceInner {
    children: RBTree<AddrSpaceChildAdapter>,
}

impl SliceInner {
    fn new() -> Self {
        Self {
            children: RBTree::default(),
        }
    }

    fn get_child(&self, vpn: VirtPageNum) -> Option<&AddrSpaceChild> {
        self.children
            .upper_bound(Bound::Included(&vpn))
            .get()
            .filter(|node| vpn < node.end())
            .map(|node| &node.data)
    }
}

enum AddrSpaceChild {
    Subslice(Arc<SliceData>),
    Mapping(Arc<MappingData>),
}

struct MappingData {
    start: VirtPageNum,
    page_count: usize,
    object_offset: usize,
    object: Arc<dyn VmObject>,
    inner: QCell<Option<MappingInner>>,
}

struct MappingInner {
    perms: PageTablePerms,
}

impl MappingInner {
    fn new(perms: PageTablePerms) -> Self {
        Self { perms }
    }
}

struct AddrSpaceChildNode {
    link: RBTreeAtomicLink,
    data: AddrSpaceChild,
}

impl AddrSpaceChildNode {
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

intrusive_adapter!(AddrSpaceChildAdapter = Box<AddrSpaceChildNode>: AddrSpaceChildNode { link: RBTreeAtomicLink });
impl<'a> KeyAdapter<'a> for AddrSpaceChildAdapter {
    type Key = VirtPageNum;

    fn get_key(&self, value: &'a AddrSpaceChildNode) -> Self::Key {
        value.start()
    }
}
