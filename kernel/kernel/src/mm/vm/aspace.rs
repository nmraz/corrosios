use alloc::sync::Arc;
use core::ops::Range;

use arrayvec::ArrayVec;
use qcell::QCellOwner;

use crate::err::{Error, Result};
use crate::mm::physmap::PhysmapPfnTranslator;
use crate::mm::pmm;
use crate::mm::pt::{GatherInvalidations, MappingPointer, PageTable, PageTableAlloc};
use crate::mm::types::{PageTablePerms, PhysFrameNum, Protection, VirtPageNum};
use crate::sync::SpinLock;

use self::tree::{Mapping, Slice, SliceChild};

use super::object::VmObject;
use super::AccessType;

mod tree;

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
    fn flush(&self, request: TlbFlush<'_>);

    /// Returns the base page table permissions for pages mapped into this address space.
    fn base_perms(&self) -> PageTablePerms;
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
    // TODO: probably don't want a spinlock here
    inner: SpinLock<AddrSpaceInner>,
    root_slice: Arc<Slice>,
    ops: O,
}

struct AddrSpaceInner {
    owner: QCellOwner,
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

        let owner = QCellOwner::new();
        let root_slice = Slice::new(
            owner.id(),
            None,
            "root",
            range.start,
            range.end - range.start,
        )?;

        Ok(AddrSpace {
            inner: SpinLock::new(AddrSpaceInner { owner }),
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
                let offset = self.0 - mapping.start();
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
        let slice = self.with_owner(|owner| {
            let id = owner.id();

            slice.slice.alloc_spot(owner, start, page_count, |start| {
                let subslice =
                    Slice::new(id, Some(Arc::clone(&slice.slice)), name, start, page_count)?;

                let child = SliceChild::Subslice(Arc::clone(&subslice));
                Ok((child, subslice))
            })
        })?;

        Ok(SliceHandle { slice })
    }

    /// Unmaps `slice` from this address space, recursively unmapping all nested mappings and
    /// subslices.
    ///
    /// When this function returns, `slice` will be detached, and any address space operations on
    /// it will return `INVALID_STATE`.
    ///
    /// # Errors
    ///
    /// * `INVALID_ARGUMENT` - This function was called on the root slice.
    /// * `INVALID_STATE` - `slice` is already detached.
    ///
    /// # Panics
    ///
    /// Panics if `slice` belongs to a different address space.
    ///
    /// # Safety
    ///
    /// * The range unmapped must not be accessed after this function returns
    pub unsafe fn unmap_slice(&self, slice: &SliceHandle) -> Result<()> {
        self.with_owner(|owner| {
            let parent = slice.slice.parent(owner)?.ok_or(Error::INVALID_ARGUMENT)?;

            parent.remove_child(owner, slice.start())?;
            slice.slice.detach_children(owner);

            unsafe {
                self.do_unmap(slice.start(), slice.page_count());
            }

            Ok(())
        })
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
    /// * `INVALID_ARGUMENT` - The requested address range is too large or does not lie in the
    ///                        virtual address range managed by this slice, or the requested offset
    ///                        range does not fit within the object.
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
        prot: Protection,
    ) -> Result<MappingHandle> {
        let total_page_count = object.page_count();

        if object_offset > total_page_count || page_count > total_page_count - object_offset {
            return Err(Error::INVALID_ARGUMENT);
        }

        let mapping = self.with_owner(|owner| {
            let id = owner.id();
            slice
                .slice
                .alloc_spot(owner, start, total_page_count, |start| {
                    let mapping = Mapping::new(
                        id,
                        Arc::clone(&slice.slice),
                        start,
                        page_count,
                        object,
                        object_offset,
                        prot,
                    )?;

                    let child = SliceChild::Mapping(Arc::clone(&mapping));
                    Ok((child, mapping))
                })
        })?;

        Ok(MappingHandle { mapping })
    }

    /// Unmaps `mapping` from this address space.
    ///
    /// When this function returns, `mapping` will be detached, and any address space operations on
    /// it will return `INVALID_STATE`.
    ///
    /// # Errors
    ///
    /// * `INVALID_STATE` - `mapping` is already detached.
    ///
    /// # Panics
    ///
    /// Panics if `mapping` belongs to a different address space.
    ///
    /// # Safety
    ///
    /// * The range unmapped must not be accessed after this function returns
    pub unsafe fn unmap(&self, mapping: &MappingHandle) -> Result<()> {
        self.with_owner(|owner| {
            let parent = mapping.mapping.parent(owner)?;
            parent.remove_child(owner, mapping.start())?;

            unsafe {
                self.do_unmap(mapping.start(), mapping.page_count());
            }

            Ok(())
        })
    }

    /// Commits `page_count` pages in `mapping`, starting at `offset`, for accesses of type
    /// `access_type`.
    ///
    /// Subsequent accesses of type `access_type` to the committed range are guaranteed not to cause
    /// page faults.
    ///
    /// # Errors
    ///
    /// * `INVALID_STATE` - This function was called on a [detached](MappingHandle#states) mapping.
    /// * `NO_PERMS` - `mapping` does not have sufficient permissions for accesses of type
    ///                `access_type`.
    ///
    /// # Panics
    ///
    /// Panics if `mapping` belongs to a different address space.
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
            let prot = mapping.prot(owner)?;

            if !access_allowed(access_type, prot) {
                return Err(Error::NO_PERMS);
            }

            let cache_mode = mapping.object().cache_mode();

            // TODO: refactor this and find some way for `provide_page` to block outside the
            // critical section
            for offset in range.offset..range.offset + range.page_count {
                let object_offset = offset + mapping.object_offset();

                let pfn = mapping.object().provide_page(object_offset, access_type)?;

                // Safety: we're holding the page table lock, and our translator and allocator perform
                // correctly.
                unsafe {
                    self.pt().map(
                        &mut PmmPageTableAlloc,
                        &mut MappingPointer::new(mapping.start() + offset, 1),
                        pfn,
                        self.perms_for_prot(prot),
                        cache_mode,
                    )?;
                };
            }

            Ok(())
        })
    }

    /// # Safety
    ///
    /// * This function must be called with the lock held
    /// * The range must not be accessed when this function returns
    /// * The page tables mapping the range must have been allocated by the PMM
    unsafe fn do_unmap(&self, start: VirtPageNum, page_count: usize) {
        let mut pt = self.pt();
        let mut gather = PendingInvalidationGather::new();

        unsafe {
            pt.unmap(&mut gather, &mut MappingPointer::new(start, page_count))
                .expect("failed to unmap page range");
            self.ops.flush(gather.as_tlb_flush());
            pt.cull_tables(|table| pmm::deallocate(table, 0), start, page_count);
        }
    }

    fn with_owner<R>(&self, f: impl FnOnce(&mut QCellOwner) -> R) -> R {
        self.inner.with(|inner, _| f(&mut inner.owner))
    }

    fn pt(&self) -> PageTable<PhysmapPfnTranslator> {
        // Safety: the physmap covers all normal memory, which is the only place we can allocate
        // page tables.
        unsafe { PageTable::new(self.ops.root_pt(), PhysmapPfnTranslator) }
    }

    fn perms_for_prot(&self, prot: Protection) -> PageTablePerms {
        let mut perms = self.ops.base_perms();

        perms.set(PageTablePerms::READ, prot.contains(Protection::READ));
        perms.set(PageTablePerms::WRITE, prot.contains(Protection::WRITE));
        perms.set(PageTablePerms::EXECUTE, prot.contains(Protection::EXECUTE));

        perms
    }
}

impl<O> Drop for AddrSpace<O> {
    fn drop(&mut self) {
        let owner = &mut self.inner.get_mut().owner;
        self.root_slice.detach_children(owner);
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
    slice: Arc<Slice>,
}

impl SliceHandle {
    /// Returns the human-friendly name of this slice, useful for debugging purposes.
    ///
    /// The root slice of an address space is always named `root`.
    pub fn name(&self) -> &str {
        self.slice.name()
    }

    /// Returns the first page number covered by this slice.
    pub fn start(&self) -> VirtPageNum {
        self.slice.start()
    }

    /// Returns the page number just after the last page covered by this slice.
    pub fn end(&self) -> VirtPageNum {
        self.slice.end()
    }

    /// Returns the number of pages covered by this slice.
    pub fn page_count(&self) -> usize {
        self.slice.page_count()
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
    mapping: Arc<Mapping>,
}

impl MappingHandle {
    /// Returns the first page number covered by this mapping.
    pub fn start(&self) -> VirtPageNum {
        self.mapping.start()
    }

    /// Returns the page number just after the last page covered by this mapping.
    pub fn end(&self) -> VirtPageNum {
        self.mapping.end()
    }

    /// Returns the number of pages covered by this mapping.
    pub fn page_count(&self) -> usize {
        self.mapping.page_count()
    }

    /// Returns the offset in the VM object at which this mapping starts.
    pub fn object_offset(&self) -> usize {
        self.mapping.object_offset()
    }

    /// Returns a handle to the underlying VM object.
    pub fn object(&self) -> &Arc<dyn VmObject> {
        self.mapping.object()
    }
}

#[derive(Clone, Copy)]
struct CommitRange<'a> {
    mapping: &'a Mapping,
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

// TODO: this value was selected at random and needs verification/tuning.
const MAX_PAGE_INVALIDATIONS: usize = 10;

enum PendingInvalidationGather {
    Specific(ArrayVec<VirtPageNum, MAX_PAGE_INVALIDATIONS>),
    All,
}

impl PendingInvalidationGather {
    fn new() -> Self {
        Self::Specific(ArrayVec::new())
    }

    fn as_tlb_flush(&self) -> TlbFlush<'_> {
        match self {
            PendingInvalidationGather::Specific(pages) => TlbFlush::Specific(pages),
            PendingInvalidationGather::All => TlbFlush::All,
        }
    }
}

impl GatherInvalidations for PendingInvalidationGather {
    fn add_tlb_flush(&mut self, vpn: VirtPageNum) {
        match self {
            PendingInvalidationGather::Specific(pages) => {
                if pages.try_push(vpn).is_err() {
                    // We've exceeded the maximum number of single-page invalidations we're willing
                    // to perform, fall back to a full flush
                    *self = Self::All;
                }
            }
            PendingInvalidationGather::All => {}
        }
    }
}

struct PmmPageTableAlloc;

impl PageTableAlloc for PmmPageTableAlloc {
    fn allocate(&mut self) -> Result<PhysFrameNum> {
        pmm::allocate(0).ok_or(Error::OUT_OF_MEMORY)
    }
}

fn access_allowed(access_type: AccessType, prot: Protection) -> bool {
    match access_type {
        AccessType::Read => prot.contains(Protection::READ),
        AccessType::Write => prot.contains(Protection::WRITE),
        AccessType::Execute => prot.contains(Protection::EXECUTE),
    }
}
