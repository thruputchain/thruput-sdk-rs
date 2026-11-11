//! A bump allocator that frees and resizes its most recent allocation.
//!
//! The runtime's default bump allocator never reuses memory. This one keeps
//! the same cursor but handles the top of the heap specially:
//!
//! - `dealloc` of the most recent allocation moves the cursor back, so
//!   short-lived buffers that are dropped in allocation order are reused;
//! - `realloc` of the most recent allocation grows or shrinks it in place,
//!   so a growing `Vec` with no allocations after it never copies.
//!
//! Every other `dealloc` is a no-op, as before. Memory is still reclaimed in
//! full only when the invocation ends and the VM resets guest memory.
//!
//! The address arithmetic is unchecked, as in the runtime's allocator, so
//! that the two differ only in reclaiming. It cannot overflow: the linker
//! script ASSERTs that the heap ends at or below 2^31, and `Layout` keeps a
//! size, rounded up to its alignment, at or below `isize::MAX`. An aligned
//! heap address plus such a size is therefore at most `usize::MAX`.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::ptr;

pub struct ReclaimingBump {
    /// The next free address, or 0 before the first allocation. The heap
    /// lies above the stack, so 0 is never a real cursor.
    cursor: Cell<usize>,
}

// SAFETY: the guest is single-threaded.
unsafe impl Sync for ReclaimingBump {}

impl ReclaimingBump {
    pub const fn new() -> Self {
        Self {
            cursor: Cell::new(0),
        }
    }

    /// The heap base pointer, carrying the heap's provenance, and the
    /// current cursor and end address.
    #[inline(always)]
    fn state(&self) -> (*mut u8, usize, usize) {
        let heap = thruput_sdk::guest_heap();
        let base = heap.cast::<u8>().as_ptr();
        let end = base.addr() + heap.len();
        debug_assert!(end <= isize::MAX as usize + 1, "heap ends above 2^31");
        let cursor = match self.cursor.get() {
            0 => base.addr(),
            c => c,
        };
        (base, cursor, end)
    }
}

unsafe impl GlobalAlloc for ReclaimingBump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (base, cursor, end) = self.state();
        let mask = layout.align() - 1;
        let start = (cursor + mask) & !mask;
        let new_cursor = start + layout.size();
        if new_cursor > end {
            return ptr::null_mut();
        }
        self.cursor.set(new_cursor);
        base.with_addr(start)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (_, cursor, _) = self.state();
        if ptr.addr() + layout.size() == cursor {
            self.cursor.set(ptr.addr());
        }
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        let (_, cursor, end) = self.state();
        let old_size = layout.size();

        // The most recent allocation: resize it in place. If it cannot grow
        // here, nothing above it is free either.
        if ptr.addr() + old_size == cursor {
            let new_cursor = ptr.addr() + new_size;
            if new_cursor > end {
                return ptr::null_mut();
            }
            self.cursor.set(new_cursor);
            return ptr;
        }

        // Anywhere else, shrinking keeps the block where it is.
        if new_size <= old_size {
            return ptr;
        }

        // SAFETY: the caller guarantees `new_size`, rounded up to
        // `layout.align()`, does not overflow `isize`.
        let new_layout = unsafe {
            Layout::from_size_align_unchecked(new_size, layout.align())
        };
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            // SAFETY: both blocks are at least `old_size` long, and the new
            // block lies above the cursor, past the old one.
            unsafe { ptr::copy_nonoverlapping(ptr, new_ptr, old_size) };
        }
        new_ptr
    }
}
