//! Bump allocator for rv32 contracts.
//!
//! Mirrors `sdk::allocator::SlabAllocator` (the WASM version) but bounds
//! against `__heap_end` from the linker script instead of the WASM page
//! size constant. `__heap_end` is the true end of guest memory: the stack
//! is parked at the bottom of DATA (stack-first layout), so the heap is the
//! topmost region. The 64 KiB guest memory cap means a bump allocator
//! is the right choice: contracts allocate small, transient values during a
//! single invocation, and the VM resets memory between invocations anyway.

use core::alloc::{GlobalAlloc, Layout};

unsafe extern "C" {
    static __heap_base: u8;
    static __heap_end: u8;
}

static mut CURSOR: usize = usize::MAX;

struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let mut cursor = unsafe { CURSOR };
        if cursor == usize::MAX {
            cursor = unsafe { &__heap_base as *const u8 as usize };
        }

        let heap_end = unsafe { &__heap_end as *const u8 as usize };

        let Some(aligned) =
            cursor.checked_add(align - 1).map(|c| c & !(align - 1))
        else {
            return core::ptr::null_mut();
        };
        let Some(new_cursor) = aligned.checked_add(size) else {
            return core::ptr::null_mut();
        };

        if new_cursor > heap_end {
            return core::ptr::null_mut();
        }

        unsafe { CURSOR = new_cursor };
        aligned as *mut u8
    }

    #[inline(always)]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static GLOBAL: BumpAllocator = BumpAllocator;
