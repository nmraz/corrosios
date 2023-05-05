use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::ptr::NonNull;
use core::{cmp, mem};

use bitmap::BorrowedBitmapMut;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use num_utils::{align_down, align_up, log2_ceil};

use super::physmap::{pfn_to_physmap, physmap_to_pfn};
use super::pmm;
use super::types::VirtAddr;
use super::utils::to_page_count;
use crate::arch::mmu::PAGE_SIZE;
use crate::sync::SpinLock;

#[global_allocator]
static RUST_ALLOCATOR: KernelHeapAlloc = KernelHeapAlloc;

#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("allocation for layout {:x?} failed", layout);
}

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocate(layout).map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr().cast())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            let ptr = NonNull::new_unchecked(ptr);
            deallocate(ptr, layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let ptr = NonNull::new_unchecked(ptr);
            let new_layout = match Layout::from_size_align(new_size, layout.align()) {
                Ok(layout) => layout,
                Err(_) => return core::ptr::null_mut(),
            };

            resize(ptr, layout, new_layout).map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr().cast())
        }
    }
}

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

// Note: the correctness of the alignment handling in the allocator above depends on the fact that
// rounding any size up to its nearest size class below preserves the largest power of 2 dividing
// the number; in other words, rounding a number up to its size class must not decrease its trailing
// zero count. We ensure this by never adding a power of 2 to a size class not already divisible by
// that power, which would cause us to "skip" a size class that was more strictly aligned.
static ALLOCATOR: Allocator<25> = Allocator::new([
    // For small marker objects like `QCellOwner`
    SizeClass::new(2, 0),
    // Single pointers and other very small objects
    SizeClass::new(8, 0),
    // 16-byte granularity
    SizeClass::new(16, 0),
    SizeClass::new(32, 0),
    SizeClass::new(48, 0),
    SizeClass::new(64, 0),
    SizeClass::new(80, 0),
    SizeClass::new(96, 0),
    // 32-byte granularity
    SizeClass::new(128, 0),
    SizeClass::new(160, 0),
    SizeClass::new(192, 0),
    SizeClass::new(224, 0),
    // 64-byte granularity
    SizeClass::new(256, 1),
    SizeClass::new(320, 1),
    SizeClass::new(384, 1),
    SizeClass::new(448, 1),
    // 128-byte granularity
    SizeClass::new(512, 2),
    SizeClass::new(640, 2),
    SizeClass::new(768, 2),
    SizeClass::new(896, 2),
    // 256-byte granularity
    SizeClass::new(1024, 3),
    SizeClass::new(1280, 3),
    SizeClass::new(1536, 3),
    SizeClass::new(1792, 3),
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
    let pages = to_page_count(bytes);
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
        // self.inner.lock().allocate(&self.meta)
        self.inner.with(|inner, _| inner.allocate(&self.meta))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>) {
        self.inner
            .with(|inner, _| unsafe { inner.deallocate(&self.meta, ptr) })
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

        while slab_header_size + bitmap::bytes_required(objects_per_slab) + objects_per_slab * size
            > slab_size
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
        bitmap::bytes_required(self.objects_per_slab)
    }
}

struct SizeClassInner {
    partial_slabs: LinkedList<SlabAdapter>,
}

impl SizeClassInner {
    fn allocate(&mut self, meta: &SizeClassMeta) -> Result<NonNull<u8>, HeapAllocError> {
        let slab = self
            .take_partial_slab()
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

        let mut bitmap = unsafe { slab_bitmap_from_header(slab, meta) };
        let offset = bitmap
            .first_zero(meta.objects_per_slab)
            .expect("no objects free in non-full slab");

        bitmap.set(offset);

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
                if meta.objects_per_slab > 1 {
                    assert!(header.link.is_linked());
                    self.partial_slabs
                        .cursor_mut_from_ptr(slab.as_ptr())
                        .remove();
                }
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
            let mut bitmap = slab_bitmap_from_header(slab, meta);
            let slab_off = ptr.as_ptr().offset_from(slab.as_ptr().cast::<u8>());
            let index = (slab_off as usize - meta.first_object_offset()) / meta.size;

            bitmap.unset(index);
        }
    }

    fn take_partial_slab(&mut self) -> Option<NonNull<SlabHeader>> {
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
) -> BorrowedBitmapMut<'a> {
    let bytes = unsafe {
        let bitmap_base = header.as_ptr().add(1).cast();
        core::slice::from_raw_parts_mut(bitmap_base, meta.bitmap_bytes())
    };
    BorrowedBitmapMut::new(bytes)
}

fn slab_header_from_obj(obj: NonNull<u8>, order: usize) -> NonNull<SlabHeader> {
    let addr = obj.as_ptr() as usize;
    let base_addr = align_down(addr, PAGE_SIZE << order);
    NonNull::new(base_addr as *mut _).expect("bad object pointer")
}

fn alloc_virt_pages(order: usize) -> Option<NonNull<u8>> {
    let pfn = pmm::allocate(order)?;
    let ptr = unsafe { NonNull::new_unchecked(pfn_to_physmap(pfn).addr().as_mut_ptr()) };
    Some(ptr)
}

unsafe fn free_virt_pages(ptr: NonNull<u8>, order: usize) {
    let vaddr = VirtAddr::from_ptr(ptr.as_ptr());
    assert_eq!(vaddr.page_offset(), 0);

    unsafe {
        pmm::deallocate(physmap_to_pfn(vaddr.containing_page()), order);
    }
}
