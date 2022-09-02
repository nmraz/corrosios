use core::alloc::Layout;
use core::cell::Cell;
use core::mem;
use core::ptr::NonNull;

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use num_utils::{align_down, align_up, div_ceil, log2_ceil};

use super::physmap::{pfn_to_physmap, physmap_to_pfn};
use super::pmm;
use super::types::VirtAddr;
use crate::arch::mmu::PAGE_SIZE;
use crate::sync::SpinLock;

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

struct SlabHeader {
    link: LinkedListLink,
    allocated: Cell<usize>,
}

intrusive_adapter!(SlabAdapter = UnsafeRef<SlabHeader>: SlabHeader { link: LinkedListLink });

struct SizeClass {
    meta: SizeClassMeta,
    inner: SpinLock<SizeClassInner>,
}

struct SizeClassMeta {
    size: usize,
    slab_order: usize,
    objects_per_slab: usize,
}

impl SizeClassMeta {
    const fn new(size: usize, slab_order: usize) -> Self {
        let slab_size = PAGE_SIZE << slab_order;
        let slab_header_size = mem::size_of::<SlabHeader>();

        let mut objects_per_slab = (slab_size - slab_header_size) / size;

        while slab_header_size + align_up(objects_per_slab, 8) + objects_per_slab * size > slab_size
        {
            objects_per_slab -= 1;
        }

        Self {
            size,
            slab_order,
            objects_per_slab,
        }
    }

    fn first_object_offset(&self) -> usize {
        (PAGE_SIZE << self.slab_order) - self.size * self.objects_per_slab
    }

    fn bitmap_bytes(&self) -> usize {
        align_up(self.objects_per_slab, 8)
    }
}

struct SizeClassInner {
    partial_slabs: LinkedList<SlabAdapter>,
}

impl SizeClassInner {
    fn allocate(&mut self, meta: &SizeClassMeta) -> Result<NonNull<u8>, HeapAllocError> {
        let slab = self
            .take_partial_slab(meta)
            .or_else(|| self.alloc_slab(meta))
            .ok_or(HeapAllocError)?;

        unsafe {
            let header = slab.as_ref();
            let next_allocated = header.allocated.get() + 1;
            if next_allocated < meta.objects_per_slab {
                self.partial_slabs.push_front(UnsafeRef::from_raw(header));
            }
        }

        let bitmap = unsafe { slab_bitmap_from_header(slab, meta) };
        let offset =
            scan_bitmap(bitmap, meta.objects_per_slab).expect("no objects free in non-full slab");

        set_bit(bitmap, offset);

        unsafe {
            let ptr = slab
                .as_ptr()
                .cast::<u8>()
                .add(meta.first_object_offset() + offset * meta.size);

            Ok(NonNull::new_unchecked(ptr))
        }
    }

    unsafe fn deallocate(&mut self, meta: &SizeClassMeta, ptr: NonNull<u8>) {
        let slab = slab_header_from_obj(ptr, meta.slab_order);
        let header = unsafe { slab.as_ref() };

        let prev_allocated = header.allocated.get();
        let next_allocated = prev_allocated - 1;

        if next_allocated == 0 {
            // If the slab is empty, don't bother updating the metadata or bitmap - just return it
            // to the PMM as-is.
            unsafe {
                free_virt_page(slab.cast(), meta.slab_order);
            }
            return;
        }

        header.allocated.set(next_allocated);

        if prev_allocated == meta.objects_per_slab && next_allocated < meta.objects_per_slab {
            // Our slab was previously full, but now has space - add it to the partial slab list.
            unsafe {
                self.partial_slabs
                    .push_front(UnsafeRef::from_raw(slab.as_ptr()));
            }
        }

        // Mark the object as free in the bitmap
        unsafe {
            let bitmap = slab_bitmap_from_header(slab, meta);
            let obj_byte_off = ptr
                .as_ptr()
                .offset_from(slab.as_ptr().cast::<u8>().add(meta.first_object_offset()));
            let obj_off = obj_byte_off as usize / meta.size;

            unset_bit(bitmap, obj_off);
        }
    }

    fn take_partial_slab(&mut self, meta: &SizeClassMeta) -> Option<NonNull<SlabHeader>> {
        self.partial_slabs
            .pop_front()
            .map(|slab| unsafe { NonNull::new_unchecked(UnsafeRef::into_raw(slab)) })
    }

    fn alloc_slab(&mut self, meta: &SizeClassMeta) -> Option<NonNull<SlabHeader>> {
        let bitmap_bytes = meta.bitmap_bytes();

        let ptr: *mut SlabHeader = alloc_virt_page(meta.slab_order)?.cast().as_ptr();

        unsafe {
            ptr.write(SlabHeader {
                link: LinkedListLink::new(),
                allocated: Cell::new(0),
            });
            ptr.add(1).cast::<u8>().write_bytes(0, bitmap_bytes);

            Some(NonNull::new_unchecked(ptr))
        }
    }
}

unsafe fn slab_bitmap_from_header<'a>(
    header: NonNull<SlabHeader>,
    meta: &SizeClassMeta,
) -> &'a mut [u8] {
    unsafe {
        let bitmap_base = header.as_ptr().add(1).cast();
        core::slice::from_raw_parts_mut(bitmap_base, meta.bitmap_bytes())
    }
}

fn slab_header_from_obj(obj: NonNull<u8>, order: usize) -> NonNull<SlabHeader> {
    let addr = obj.as_ptr() as usize;
    let base_addr = align_down(addr, PAGE_SIZE << order);
    NonNull::new(base_addr as *mut _).expect("bad object pointer")
}

fn scan_bitmap(bitmap: &[u8], limit: usize) -> Option<usize> {
    (0..limit).find(|&index| !get_bit(bitmap, index))
}

fn get_bit(bitmap: &[u8], index: usize) -> bool {
    let byte = index / 8;
    let bit = index % 8;

    ((bitmap[byte] >> bit) & 1) != 0
}

fn set_bit(bitmap: &mut [u8], index: usize) {
    let byte = index / 8;
    let bit = index % 8;

    bitmap[byte] |= 1 << bit;
}

fn unset_bit(bitmap: &mut [u8], index: usize) {
    let byte = index / 8;
    let bit = index % 8;

    bitmap[byte] &= !(1u8 << bit);
}

fn alloc_virt_page(order: usize) -> Option<NonNull<u8>> {
    let pfn = pmm::with(|pmm| pmm.allocate(order))?;
    let ptr = unsafe { NonNull::new_unchecked(pfn_to_physmap(pfn).addr().as_mut_ptr()) };
    Some(ptr)
}

unsafe fn free_virt_page(ptr: NonNull<u8>, order: usize) {
    let vaddr = VirtAddr::from_ptr(ptr.as_ptr());
    assert_eq!(vaddr.page_offset(), 0);

    pmm::with(|pmm| unsafe { pmm.deallocate(physmap_to_pfn(vaddr.containing_page()), order) });
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
