use alloc::sync::Arc;
use spin_once::Once;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END, PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};
use crate::arch::mmu::{flush_kernel_tlb, flush_kernel_tlb_page, kernel_pt_root};
use crate::err::Result;
use crate::kimage;
use crate::mm::types::{AccessType, CacheMode, PageTablePerms, PhysFrameNum, Protection, VirtAddr};

use super::aspace::{AddrSpace, AddrSpaceOps, MappingHandle, TlbFlush};
use super::object::{PhysVmObject, VmObject};

/// An owned pointer to a mapping of a VM object into the kernel address space.
pub struct KernelMapping(MappingHandle);

impl KernelMapping {
    /// Returns the base address of the mapping.
    pub fn addr(&self) -> VirtAddr {
        self.0.start().addr()
    }
}

impl Drop for KernelMapping {
    fn drop(&mut self) {
        // Safety: we have unique ownership of
        unsafe {
            get()
                .unmap(&self.0)
                .expect("kernel mapping already detached");
        }
    }
}

/// Maps the entirety of `object` into the kernel address space with protection `prot`.
pub fn kmap(object: Arc<dyn VmObject>, prot: Protection) -> Result<KernelMapping> {
    let page_count = object.page_count();

    let kernel_aspace = get();
    let mapping = kernel_aspace.map(
        &kernel_aspace.root_slice(),
        None,
        page_count,
        0,
        object,
        prot,
    )?;

    let commit_type = if prot.contains(Protection::WRITE) {
        AccessType::Write
    } else {
        AccessType::Read
    };

    kernel_aspace.commit(&mapping, commit_type, 0, page_count)?;

    Ok(KernelMapping(mapping))
}

/// Maps the physical memory range `base..base + page_count` into the kernel address space with
/// protection `prot` and cache mode `cache_mode`.
///
/// # Safety
///
/// The caller must guarantee that the specified range of physical memory is safe to access with
/// the specified cache mode, respecting any platform limitations.
pub unsafe fn iomap(
    base: PhysFrameNum,
    page_count: usize,
    prot: Protection,
    cache_mode: CacheMode,
) -> Result<KernelMapping> {
    // Safety: function contract
    let object = unsafe { PhysVmObject::new(base, page_count, cache_mode)? };
    kmap(object, prot)
}

/// Initializes the (higher-half) kernel address space.
///
/// # Panics
///
/// Panics if this function is called more than once.
pub(super) fn init() {
    let aspace = unsafe {
        AddrSpace::new(KERNEL_ASPACE_BASE..KERNEL_ASPACE_END, KernelAddrSpaceOps)
            .expect("failed to create kernel address space")
    };

    let root_slice = aspace.root_slice();

    aspace
        .create_subslice(
            &root_slice,
            "physmap",
            Some(PHYS_MAP_BASE),
            PHYS_MAP_MAX_PAGES,
        )
        .expect("failed to reserve physmap virtual address space");

    aspace
        .create_subslice(
            &root_slice,
            "kimage",
            Some(kimage::virt_base()),
            kimage::total_pages(),
        )
        .expect("failed to reserve kernel image virtual address space");

    KERNEL_ASPACE.init(aspace);
}

/// Retrieves the global kernel address space.
///
/// # Panics
///
/// Panics if [`init`] has not yet been called.
pub(super) fn get() -> &'static AddrSpace<impl AddrSpaceOps> {
    KERNEL_ASPACE
        .get()
        .expect("kernel address space not initialized")
}

struct KernelAddrSpaceOps;

unsafe impl AddrSpaceOps for KernelAddrSpaceOps {
    fn root_pt(&self) -> PhysFrameNum {
        kernel_pt_root()
    }

    fn flush(&self, request: TlbFlush<'_>) {
        match request {
            TlbFlush::Specific(pages) => {
                for &vpn in pages {
                    flush_kernel_tlb_page(vpn);
                }
            }
            TlbFlush::All => flush_kernel_tlb(),
        }
    }

    fn base_perms(&self) -> PageTablePerms {
        PageTablePerms::empty()
    }
}

static KERNEL_ASPACE: Once<AddrSpace<KernelAddrSpaceOps>> = Once::new();
