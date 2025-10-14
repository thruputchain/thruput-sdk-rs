//! An allocator that panics on alloc/dealloc

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        contract_panic!("Called alloc on allocator_panic");
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        contract_panic!("Called dealloc on allocator_panic");
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator;
