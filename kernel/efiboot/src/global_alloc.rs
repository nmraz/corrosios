use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static ALLOCATOR: PanickingAlloc = PanickingAlloc;

#[alloc_error_handler]
fn handle_alloc_error(_layout: Layout) -> ! {
    panic!()
}

struct PanickingAlloc;

unsafe impl GlobalAlloc for PanickingAlloc {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        panic!("global `alloc()` called")
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("global `dealloc()` called")
    }
}
