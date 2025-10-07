//! An allocator that panics on alloc/dealloc

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        unreachable!()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        unreachable!();
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;
