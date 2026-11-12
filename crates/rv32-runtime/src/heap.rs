//! The guest heap region, `[__heap_base, __heap_end)`, for the
//! `#[global_allocator]`.

use core::ptr::{self, NonNull};

// `__heap_base` is a maximal rv32 array so derived pointers carry provenance
// over any heap size; extern declarations reserve no storage.
unsafe extern "C" {
    static mut __heap_base: [u8; u32::MAX as usize >> 1];
    static __heap_end: u8;
}

/// The guest heap: from the end of the statics to the end of guest memory,
/// fresh for every invocation.
///
/// It belongs to the global allocator: the runtime's under
/// `thruput-sdk/alloc`, the contract's under `alloc-api` alone.
#[inline(always)]
pub fn heap() -> NonNull<[u8]> {
    let base = (&raw mut __heap_base).cast::<u8>();
    // An address one past guest memory, not an object: never form `&`.
    let end = (&raw const __heap_end) as usize;
    // The linker script ASSERTs `__heap_base <= __heap_end`.
    let len = end - base as usize;
    // SAFETY: the stack occupies the bottom of guest memory, so
    // `__heap_base` is above it and nonzero.
    unsafe { NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(base, len)) }
}
