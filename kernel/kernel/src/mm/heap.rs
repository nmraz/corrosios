use core::alloc::Layout;
use core::ptr::NonNull;

use num_utils::{align_up, div_ceil, log2_ceil};

use super::physmap::{pfn_to_physmap, physmap_to_pfn};
use super::pmm;
use super::types::VirtAddr;
use crate::arch::mmu::PAGE_SIZE;

#[derive(Debug, Clone, Copy)]
pub struct HeapAllocError;

pub fn allocate(layout: Layout) -> Result<NonNull<[u8]>, HeapAllocError> {
    if let Some(order) = large_alloc_order(layout) {
        let pfn = pmm::with(|pmm| pmm.allocate(order)).ok_or(HeapAllocError)?;
        let ptr = core::ptr::slice_from_raw_parts_mut(
            pfn_to_physmap(pfn).addr().as_mut_ptr(),
            PAGE_SIZE << order,
        );

        return Ok(NonNull::new(ptr).unwrap());
    }

    todo!()
}

pub unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
    if let Some(order) = large_alloc_order(layout) {
        let vaddr = VirtAddr::from_ptr(ptr.as_ptr());
        assert_eq!(vaddr.page_offset(), 0);

        pmm::with(|pmm| unsafe { pmm.deallocate(physmap_to_pfn(vaddr.containing_page()), order) });
    } else {
        todo!()
    }
}

fn large_alloc_order(layout: Layout) -> Option<usize> {
    let effective_size = align_up(layout.size(), layout.align());

    if effective_size >= PAGE_SIZE {
        let pages = div_ceil(effective_size, PAGE_SIZE);
        Some(log2_ceil(pages))
    } else {
        None
    }
}
