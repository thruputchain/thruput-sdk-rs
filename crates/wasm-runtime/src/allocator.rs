//! A fixed-size slab bump allocator that does not use memory.grow.
//!
//! Allocates from __heap_base to the end of the single wasm page.

use core::alloc::{GlobalAlloc, Layout};

const WASM_PAGE_SIZE: usize = 65536;

// Declared as an array spanning the whole page, not a bare `u8`, so pointers
// derived from this symbol carry provenance over the region the allocator
// actually hands out. `WASM_PAGE_SIZE` is an upper bound on the heap (which
// starts at `__heap_base > 0`), so the declared extent is a superset of the
// real one; the bounds check below keeps every returned pointer inside real
// memory. This is a declaration, not a definition: it reserves no storage and
// leaves the module's `(memory N)` page count unchanged.
unsafe extern "C" {
    static __heap_base: [u8; WASM_PAGE_SIZE];
}

static mut CURSOR: usize = usize::MAX;

struct SlabAllocator;

unsafe impl GlobalAlloc for SlabAllocator {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut cursor = unsafe { CURSOR };
        if cursor == usize::MAX {
            cursor = (&raw const __heap_base).cast::<u8>() as usize;
        }

        let Some((start, new_cursor)) =
            crate::bump::bump(cursor, WASM_PAGE_SIZE, layout)
        else {
            return core::ptr::null_mut();
        };

        unsafe { CURSOR = new_cursor };
        start as *mut u8
    }

    #[inline(always)]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static GLOBAL: SlabAllocator = SlabAllocator;
