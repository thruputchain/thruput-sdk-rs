//! Bump allocator for rv32 contracts.
//!
//! Allocates from [`crate::heap()`], the region between the statics and
//! `__memory_size`, the true end of guest memory: the stack is parked at the
//! bottom of memory (stack-first layout), so the heap is the topmost region.
//! The small guest memory (64 KiB unless the contract declares more) means a
//! bump allocator is the right default: contracts allocate small, transient
//! values during a single invocation, and the VM resets memory between
//! invocations anyway.
//!
//! A contract that wants a different allocator enables
//! `thruput-sdk/alloc-api` instead of `alloc`, which leaves this module out,
//! and declares its own `#[global_allocator]`.

use core::alloc::{GlobalAlloc, Layout};

static mut CURSOR: usize = usize::MAX;

struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let heap = crate::heap();
        let base = heap.cast::<u8>().as_ptr();
        let heap_end = base as usize + heap.len();

        let mut cursor = unsafe { CURSOR };
        if cursor == usize::MAX {
            cursor = base as usize;
        }

        let Some((start, new_cursor)) =
            crate::bump::bump(cursor, heap_end, layout)
        else {
            return core::ptr::null_mut();
        };

        unsafe { CURSOR = new_cursor };
        // Derive the result from `base` so it keeps the heap's provenance.
        base.wrapping_add(start - base as usize)
    }

    #[inline(always)]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static GLOBAL: BumpAllocator = BumpAllocator;
