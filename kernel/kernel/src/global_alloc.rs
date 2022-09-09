use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

use crate::mm::heap;

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match heap::allocate(layout) {
            Ok(ptr) => ptr.as_ptr().cast(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            let ptr = NonNull::new_unchecked(ptr);
            heap::deallocate(ptr, layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let ptr = NonNull::new_unchecked(ptr);
            let new_layout = match Layout::from_size_align(new_size, layout.align()) {
                Ok(layout) => layout,
                Err(_) => return core::ptr::null_mut(),
            };

            match heap::resize(ptr, layout, new_layout) {
                Ok(ptr) => ptr.as_ptr().cast(),
                Err(_) => core::ptr::null_mut(),
            }
        }
    }
}

#[global_allocator]
static ALLOCATOR: KernelHeapAlloc = KernelHeapAlloc;

#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("allocation for layout {:x?} failed", layout);
}
