//! A fixed-size slab bump allocator that does not use memory.grow.
//!
//! Allocates from __heap_base to the end of the single wasm page.

use core::alloc::{GlobalAlloc, Layout};

const WASM_PAGE_SIZE: usize = 65536;

unsafe extern "C" {
    static __heap_base: u8;
}

static mut CURSOR: usize = usize::MAX;

struct SlabAllocator;

unsafe impl GlobalAlloc for SlabAllocator {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let mut cursor = unsafe { CURSOR };
        if cursor == usize::MAX {
            cursor = (&raw const __heap_base) as usize;
        }

        let aligned = (cursor + align - 1) & !(align - 1);
        let new_cursor = aligned + size;

        if new_cursor > WASM_PAGE_SIZE {
            return core::ptr::null_mut();
        }

        unsafe { CURSOR = new_cursor };
        aligned as *mut u8
    }

    #[inline(always)]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static GLOBAL: SlabAllocator = SlabAllocator;
