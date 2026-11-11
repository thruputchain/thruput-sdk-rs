//! The guest heap region, for whichever `#[global_allocator]` the contract
//! links.
//!
//! The linker script places the heap above the statics and bounds it at
//! `__memory_size`, the end of guest memory: `[__heap_base, __heap_end)`.
//! The runtime's bump allocator (feature `alloc`) and a contract's own
//! allocator (`thruput-sdk/alloc-api`) both take their memory from here.

use core::ptr::{self, NonNull};

/// Provenance footprint of the heap: `__heap_base` is typed as an array this
/// large so that every pointer derived from an address inside the heap lies
/// within a declared object, whatever `__memory_size` a contract sets. It is
/// the largest object size Rust admits, so no guest memory a 32-bit VM can
/// provision outgrows it. The functional bound is `__heap_end`, read from the
/// linker at runtime; an extern declaration reserves no storage, so the size
/// costs nothing.
const HEAP_FOOTPRINT_BYTES: usize = usize::MAX >> 1;

// `__heap_base` is declared as a mutable array rather than a bare `u8` so
// pointers derived from it carry write provenance over the whole heap.
// `__heap_end` stays a bare symbol: it is absolute (`= __memory_size`), so
// only its address is ever wanted, never a byte at that address. Both are
// declarations and reserve no storage.
unsafe extern "C" {
    static mut __heap_base: [u8; HEAP_FOOTPRINT_BYTES];
    static __heap_end: u8;
}

/// The guest heap: every byte between the end of the statics and the end of
/// guest memory.
///
/// The pointer carries provenance over the whole region, so an allocator may
/// hand out any sub-range of it. The VM provisions fresh memory for every
/// invocation, so the region starts unused each time. Allocator state kept in
/// statics is reset along with it.
///
/// The heap belongs to the `#[global_allocator]`. With `thruput-sdk/alloc`
/// the runtime's bump allocator owns it, so other code must not write to it.
/// With `thruput-sdk/alloc-api` alone, the contract's allocator owns it.
///
/// The length is `__memory_size` less the stack reserve and the statics. A
/// larger `initial_memory_bytes` or a smaller `__stack_size` enlarges it; see
/// the crate docs.
#[inline(always)]
pub fn heap() -> NonNull<[u8]> {
    let base = (&raw mut __heap_base).cast::<u8>();
    // `__heap_end` names an address one past the end of guest memory rather
    // than a real object. Take its address without forming a reference:
    // `&__heap_end` would assert a valid, dereferenceable byte there.
    let end = (&raw const __heap_end) as usize;
    // The linker script ASSERTs `__heap_base <= __heap_end`, so this cannot
    // underflow.
    let len = end - base as usize;
    // SAFETY: the stack occupies the bottom of guest memory, so
    // `__heap_base` is above it and nonzero.
    unsafe { NonNull::new_unchecked(ptr::slice_from_raw_parts_mut(base, len)) }
}
