//! Bump allocator for rv32 contracts.
//!
//! Bounds allocation against `__heap_end` from the linker script.
//! `__heap_end` is pinned to `__memory_size`, the true end
//! of guest memory (intentionally smaller than the linker script's relaxed
//! DATA region): the stack is parked at the bottom of memory (stack-first
//! layout), so the heap is the topmost region. The 64 KiB guest memory cap
//! means a bump allocator
//! is the right choice: contracts allocate small, transient values during a
//! single invocation, and the VM resets memory between invocations anyway.

use core::alloc::{GlobalAlloc, Layout};

/// Upper bound on the heap extent, mirroring `__memory_size` in
/// `link/groundhog-rv32.ld`. Used *only* to give `__heap_base` a provenance
/// footprint; the functional bound is `__heap_end`, read from the linker at
/// runtime. Drift against the linker script is therefore not a safety issue:
/// too small merely narrows provenance, too large over-covers.
const GUEST_MEMORY_SIZE: usize = 64 * 1024;

// `__heap_base` is declared as an array rather than a bare `u8` so pointers
// derived from it carry provenance over the region the allocator hands out.
// `__heap_end` stays a bare symbol: it is absolute (`= __memory_size`), so only
// its address is ever wanted, never a byte at that address. Both are
// declarations and reserve no storage.
unsafe extern "C" {
    static __heap_base: [u8; GUEST_MEMORY_SIZE];
    static __heap_end: u8;
}

static mut CURSOR: usize = usize::MAX;

struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut cursor = unsafe { CURSOR };
        if cursor == usize::MAX {
            cursor = (&raw const __heap_base).cast::<u8>() as usize;
        }

        // `__heap_end` is an absolute linker symbol (`= __memory_size`), so it
        // names an address one past the end of guest memory rather than a real
        // object. Take its address without forming a reference: `&__heap_end`
        // would assert a valid, dereferenceable byte that does not exist.
        let heap_end = (&raw const __heap_end) as usize;

        let Some((start, new_cursor)) =
            crate::bump::bump(cursor, heap_end, layout)
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
static GLOBAL: BumpAllocator = BumpAllocator;
