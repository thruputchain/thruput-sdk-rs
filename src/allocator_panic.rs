use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        0 as *mut u8
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        unreachable!();
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;
