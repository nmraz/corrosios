use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::NonNull;
use core::{cmp, mem};

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
    ALLOCATOR.allocate(get_effective_size(layout))
}

pub unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
    unsafe { ALLOCATOR.deallocate(ptr, get_effective_size(layout)) }
}

pub unsafe fn resize(
    ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
) -> Result<NonNull<[u8]>, HeapAllocError> {
    let old_effective_size = get_effective_size(old_layout);
    let new_effective_size = get_effective_size(new_layout);

    let old_usable_size = ALLOCATOR.usable_size(old_effective_size);
    let new_usable_size = ALLOCATOR.usable_size(new_effective_size);

    if old_usable_size == new_usable_size {
        Ok(nonnull_slice_from_raw_parts(ptr, old_usable_size))
    } else {
        let new_ptr = ALLOCATOR.allocate(new_effective_size)?;
        let copy_size = cmp::min(old_layout.size(), new_layout.size());

        unsafe {
            new_ptr
                .as_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(ptr.as_ptr(), copy_size);
            ALLOCATOR.deallocate(ptr, old_effective_size);
        }

        Ok(new_ptr)
    }
}

fn get_effective_size(layout: Layout) -> usize {
    align_up(layout.size(), layout.align())
}

static ALLOCATOR: Allocator<23> = Allocator::new([
    SizeClass::new(16, 0),
    SizeClass::new(32, 0),
    SizeClass::new(48, 0),
    SizeClass::new(64, 0),
    SizeClass::new(80, 0),
    SizeClass::new(96, 0),
    SizeClass::new(128, 0),
    SizeClass::new(160, 0),
    SizeClass::new(192, 0),
    SizeClass::new(224, 1),
    SizeClass::new(256, 1),
    SizeClass::new(320, 1),
    SizeClass::new(384, 1),
    SizeClass::new(448, 2),
    SizeClass::new(512, 2),
    SizeClass::new(640, 2),
    SizeClass::new(768, 2),
    SizeClass::new(896, 2),
    SizeClass::new(1024, 2),
    SizeClass::new(1280, 2),
    SizeClass::new(1536, 2),
    SizeClass::new(1792, 2),
    SizeClass::new(2048, 3),
]);

struct Allocator<const N: usize> {
    size_classes: [SizeClass; N],
}

impl<const N: usize> Allocator<N> {
    const fn new(size_classes: [SizeClass; N]) -> Self {
        Self { size_classes }
    }

    fn allocate(&self, effective_size: usize) -> Result<NonNull<[u8]>, HeapAllocError> {
        match self.get_size_class(effective_size) {
            Some(size_class) => {
                let ptr = size_class.allocate()?;
                Ok(nonnull_slice_from_raw_parts(ptr, size_class.size()))
            }
            None => {
                // Request too large for the slab allocator, get pages directly from the PMM
                let order = raw_page_order(effective_size);
                let ptr = alloc_virt_pages(order).ok_or(HeapAllocError)?;
                Ok(nonnull_slice_from_raw_parts(ptr, PAGE_SIZE << order))
            }
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, effective_size: usize) {
        match self.get_size_class(effective_size) {
            Some(size_class) => unsafe {
                size_class.deallocate(ptr);
            },
            None => {
                let order = raw_page_order(effective_size);
                unsafe {
                    free_virt_pages(ptr, order);
                }
            }
        }
    }

    unsafe fn try_resize_in_place(
        &self,
        ptr: NonNull<u8>,
        old_effective_size: usize,
        new_effective_size: usize,
    ) -> Result<usize, HeapAllocError> {
        let old_usable_size = self.usable_size(old_effective_size);
        if old_usable_size == self.usable_size(new_effective_size) {
            return Ok(old_usable_size);
        }

        Err(HeapAllocError)
    }

    fn usable_size(&self, effective_size: usize) -> usize {
        match self.get_size_class(effective_size) {
            Some(size_class) => size_class.size(),
            None => PAGE_SIZE << raw_page_order(effective_size),
        }
    }

    fn get_size_class(&self, effective_size: usize) -> Option<&SizeClass> {
        let i = self
            .size_classes
            .binary_search_by_key(&effective_size, |size_class| size_class.size())
            .unwrap_or_else(|i| i);

        self.size_classes.get(i)
    }
}

fn nonnull_slice_from_raw_parts<T>(data: NonNull<T>, len: usize) -> NonNull<[T]> {
    unsafe { NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(data.as_ptr(), len)) }
}

fn raw_page_order(bytes: usize) -> usize {
    let pages = div_ceil(bytes, PAGE_SIZE);
    log2_ceil(pages)
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

impl SizeClass {
    const fn new(size: usize, slab_order: usize) -> Self {
        Self {
            meta: SizeClassMeta::new(size, slab_order),
            inner: SpinLock::new(SizeClassInner {
                partial_slabs: LinkedList::new(SlabAdapter::NEW),
            }),
        }
    }

    fn size(&self) -> usize {
        self.meta.size
    }

    fn allocate(&self) -> Result<NonNull<u8>, HeapAllocError> {
        self.inner.lock().allocate(&self.meta)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>) {
        unsafe { self.inner.lock().deallocate(&self.meta, ptr) }
    }
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
            header.allocated.set(next_allocated);
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
                free_virt_pages(slab.cast(), meta.slab_order);
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
            let slab_off = ptr.as_ptr().offset_from(slab.as_ptr().cast::<u8>());
            let index = (slab_off as usize - meta.first_object_offset()) / meta.size;

            unset_bit(bitmap, index);
        }
    }

    fn take_partial_slab(&mut self, meta: &SizeClassMeta) -> Option<NonNull<SlabHeader>> {
        self.partial_slabs
            .pop_front()
            .map(|slab| unsafe { NonNull::new_unchecked(UnsafeRef::into_raw(slab)) })
    }

    fn alloc_slab(&mut self, meta: &SizeClassMeta) -> Option<NonNull<SlabHeader>> {
        let bitmap_bytes = meta.bitmap_bytes();

        let ptr: *mut SlabHeader = alloc_virt_pages(meta.slab_order)?.cast().as_ptr();

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

fn alloc_virt_pages(order: usize) -> Option<NonNull<u8>> {
    let pfn = pmm::with(|pmm| pmm.allocate(order))?;
    let ptr = unsafe { NonNull::new_unchecked(pfn_to_physmap(pfn).addr().as_mut_ptr()) };
    Some(ptr)
}

unsafe fn free_virt_pages(ptr: NonNull<u8>, order: usize) {
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
