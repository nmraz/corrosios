use alloc::sync::Arc;

use crate::arch::mmu::PAGE_SIZE;
use crate::err::Result;

use super::types::{CacheMode, PhysAddr, Protection, VirtAddr};
use super::utils::to_page_count;
use super::vm;
use super::vm::aspace::{MapBase, MappingHandle, SliceHandle};
use super::vm::object::{EagerVmObject, PhysVmObject, VmObject};

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
        // Safety: we have unique ownership of the mapping.
        unsafe {
            vm::get_kernel_addr_space()
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

const STACK_SIZE: usize = 0x8000;
const STACK_PAGES: usize = STACK_SIZE / PAGE_SIZE;

pub struct KernelStack {
    slice: SliceHandle,
}

impl KernelStack {
    pub fn new() -> Result<Self> {
        let kernel_aspace = vm::get_kernel_addr_space();

        let stack_obj = EagerVmObject::new(STACK_PAGES)?;
        let slice = kernel_aspace.create_subslice(
            kernel_aspace.root_slice(),
            "kernel stack",
            MapBase::any(),
            STACK_PAGES + 1,
        )?;

        let stack = KernelStack { slice };

        // Leave a guard page at the bottom of the stack.
        kernel_aspace.map_committed(
            &stack.slice,
            MapBase::Fixed(stack.slice.start() + 1),
            STACK_PAGES,
            0,
            stack_obj,
            Protection::READ | Protection::WRITE,
        )?;

        Ok(stack)
    }

    pub fn top(&self) -> VirtAddr {
        self.slice.end().addr()
    }

    pub fn bottom(&self) -> VirtAddr {
        self.slice.start().addr()
    }

    pub fn guard_page_contains(&self, addr: VirtAddr) -> bool {
        let bottom = self.slice.start();
        (bottom.addr()..(bottom + 1).addr()).contains(&addr)
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // Safety: we have unique ownership of the stack slice.
        unsafe {
            vm::get_kernel_addr_space()
                .unmap_slice(&self.slice)
                .expect("failed to unmap kernel stack");
        }
    }
}

/// Maps the entirety of `object` into the kernel address space with protection `prot`.
pub fn kmap(object: Arc<dyn VmObject>, prot: Protection) -> Result<KernelMapping> {
    let page_count = object.page_count();

    let kernel_aspace = vm::get_kernel_addr_space();
    let mapping = kernel_aspace.map_committed(
        kernel_aspace.root_slice(),
        MapBase::any(),
        page_count,
        0,
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
