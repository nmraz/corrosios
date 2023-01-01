use alloc::sync::Arc;

use crate::err::Result;

use super::types::{CacheMode, PhysAddr, Protection, VirtAddr};
use super::utils::to_page_count;
use super::vm::aspace::{AddrSpace, AddrSpaceOps, MappingHandle, SliceHandle};
use super::vm::kernel_aspace;
use super::vm::object::{PhysVmObject, VmObject};

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
            kernel_aspace::get()
                .unmap(&self.0)
                .expect("kernel mapping already detached");
        }
    }
}

pub struct IoMapping {
    mapping: KernelMapping,
    page_offset: usize,
    len: usize,
}

impl IoMapping {
    pub fn addr(&self) -> VirtAddr {
        self.mapping.addr() + self.page_offset
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

/// Maps the entirety of `object` into the kernel address space with protection `prot`.
pub fn kmap(object: Arc<dyn VmObject>, prot: Protection) -> Result<KernelMapping> {
    let page_count = object.page_count();

    let kernel_aspace = kernel_aspace::get();
    let mapping = map_committed(
        kernel_aspace,
        &kernel_aspace.root_slice(),
        page_count,
        object,
        prot,
    )?;

    Ok(KernelMapping(mapping))
}

/// Maps the physical byte range `base..base + len` into the kernel address space with protection
/// `prot` and cache mode `cache_mode`.
///
/// # Safety
///
/// The caller must guarantee that the specified range of physical memory is safe to access with
/// the specified cache mode, respecting any platform limitations.
pub unsafe fn iomap(
    base: PhysAddr,
    len: usize,
    prot: Protection,
    cache_mode: CacheMode,
) -> Result<IoMapping> {
    let base_pfn = base.containing_frame();
    let page_offset = base.frame_offset();

    // Safety: function contract
    let object = unsafe { PhysVmObject::new(base_pfn, to_page_count(len), cache_mode)? };
    let mapping = kmap(object, prot)?;

    Ok(IoMapping {
        mapping,
        page_offset,
        len,
    })
}

fn map_committed(
    kernel_aspace: &AddrSpace<impl AddrSpaceOps>,
    slice: &SliceHandle,
    page_count: usize,
    object: Arc<dyn VmObject>,
    prot: Protection,
) -> Result<MappingHandle> {
    let mapping = kernel_aspace.map(slice, None, page_count, 0, object, prot)?;
    kernel_aspace.commit(&mapping, 0, page_count)?;
    Ok(mapping)
}
