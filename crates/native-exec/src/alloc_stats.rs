//! Per-invocation-frame allocation statistics for native contract tests.
//!
//! When contract code runs natively, its heap usage approximates what the
//! rv32 guest's bump allocator would consume (`rv32-runtime`'s allocator
//! never frees and reallocs by fresh alloc + copy). This module counts
//! allocations on the current thread and attributes them to invocation
//! frames *exclusively*: each frame is charged only for its own
//! allocations, not its callees', matching the real system where every
//! frame is a separate guest instance with its own heap.
//!
//! Stats are only collected when the test binary installs the counting
//! allocator:
//!
//! ```rust,ignore
//! groundhog_native_exec::install_counting_allocator!();
//! ```
//!
//! Without it, everything runs normally and all stats read zero.
//!
//! Results ride on [`crate::frame_result::FrameResult`], next to the
//! frame's trace and events: `alloc_stats()` is the frame's own
//! (exclusive) usage, and `cross_call_alloc_records()` lists one
//! [`FrameAllocRecord`] per cross-call (`client.foo()`-style) invocation
//! that completed during the frame's execution, in completion order.
//!
//! # Approximations
//!
//! Native numbers are a proxy for guest heap consumption, good for
//! relative comparisons and regression bounds — not exact budget math:
//!
//! - Native `usize`/pointers are 8 bytes; rv32's are 4.
//! - The measured window includes small harness allocations (e.g.
//!   `FrameBuilder::run`'s XDR-encode of the return value, which on the
//!   guest uses the no-alloc scratch buffer instead).
//! - Panic unwinding (abort paths, `raw_Return`'s control-flow unwind)
//!   allocates its payload box natively.

use groundhog_xdr::contract::ContractEntrypointID;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

/// Allocation counters for one invocation frame (exclusive of callees).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllocStats {
    /// Number of allocation calls (including the alloc half of reallocs).
    pub allocs: u64,
    /// Cumulative bytes requested. Never decremented: the rv32 bump
    /// allocator never reuses freed memory, so cumulative is the
    /// budget-relevant metric.
    pub bytes_total: u64,
    /// Simulated rv32 bump-allocator consumption: replays the guest
    /// allocator's cursor arithmetic (align-up, then advance by size)
    /// for the same allocation sequence, so alignment padding is
    /// included.
    pub heap_used: u64,
}

impl AllocStats {
    pub const ZERO: AllocStats = AllocStats {
        allocs: 0,
        bytes_total: 0,
        heap_used: 0,
    };
}

/// Stats for one cross-call (trampoline) invocation of a registered mock
/// contract method, i.e. one `client.foo()` frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameAllocRecord {
    /// Index of the mock contract (`Contract::kMockTest { mock_contract_id }`).
    pub mock_contract_index: i32,
    /// Entrypoint that was invoked.
    pub entrypoint: ContractEntrypointID,
    /// Exclusive allocation stats for the frame.
    pub stats: AllocStats,
}

// Monotonic per-thread counters. Const-initialized `Cell`s: lazy TLS
// initialization can allocate, which would recurse into the counting
// allocator, and `Cell<u64>` has no destructor so nothing is registered
// at first touch either.
thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static BYTES_TOTAL: Cell<u64> = const { Cell::new(0) };
    // Simulated rv32 bump cursor (see `AllocStats::heap_used`).
    static SIM_CURSOR: Cell<u64> = const { Cell::new(0) };
    // Sum of the inclusive stats of completed frame scopes; lets an
    // enclosing scope subtract its direct children (see
    // `FrameMeasureScope::close`).
    static CHILD_CHARGED: Cell<AllocStats> = const { Cell::new(AllocStats::ZERO) };
    static FRAME_RECORDS: RefCell<Vec<FrameAllocRecord>> =
        const { RefCell::new(Vec::new()) };
}

/// Record one allocation of `size` bytes with `align` alignment into the
/// current thread's counters. Called by the counting allocator; exposed
/// to unit tests for deterministic sequences.
pub(crate) fn record_alloc(size: usize, align: usize) {
    ALLOCS.with(|c| c.set(c.get() + 1));
    BYTES_TOTAL.with(|c| c.set(c.get() + size as u64));
    SIM_CURSOR.with(|c| {
        // The rv32 bump allocator's arithmetic: align the cursor up,
        // then advance by the request size.
        let align = align as u64;
        let aligned = (c.get() + (align - 1)) & !(align - 1);
        c.set(aligned + size as u64);
    });
}

/// Snapshot of the thread's monotonic totals since thread start.
pub(crate) fn totals() -> AllocStats {
    AllocStats {
        allocs: ALLOCS.with(Cell::get),
        bytes_total: BYTES_TOTAL.with(Cell::get),
        heap_used: SIM_CURSOR.with(Cell::get),
    }
}

impl core::ops::Sub for AllocStats {
    type Output = AllocStats;
    fn sub(self, rhs: AllocStats) -> AllocStats {
        AllocStats {
            allocs: self.allocs - rhs.allocs,
            bytes_total: self.bytes_total - rhs.bytes_total,
            heap_used: self.heap_used - rhs.heap_used,
        }
    }
}

impl core::ops::Add for AllocStats {
    type Output = AllocStats;
    fn add(self, rhs: AllocStats) -> AllocStats {
        AllocStats {
            allocs: self.allocs + rhs.allocs,
            bytes_total: self.bytes_total + rhs.bytes_total,
            heap_used: self.heap_used + rhs.heap_used,
        }
    }
}

/// Measurement scope for one frame window. `open` before running frame
/// code, `close` after; `close` returns the frame's *exclusive* stats
/// (nested scopes' allocations are subtracted).
pub(crate) struct FrameMeasureScope {
    start: AllocStats,
    child_start: AllocStats,
}

impl FrameMeasureScope {
    pub(crate) fn open() -> Self {
        Self {
            start: totals(),
            child_start: CHILD_CHARGED.with(Cell::get),
        }
    }

    pub(crate) fn close(self) -> AllocStats {
        let inclusive = totals() - self.start;
        let children = CHILD_CHARGED.with(Cell::get) - self.child_start;
        // Report this whole window (not just our exclusive share) as
        // charged, *replacing* any grandchild contributions: the
        // enclosing scope must subtract exactly the inclusive stats of
        // its direct children, no more.
        CHILD_CHARGED.with(|c| c.set(self.child_start + inclusive));
        inclusive - children
    }
}

/// Append a cross-call frame record to the current thread's log. The
/// enclosing frame's `execute` window claims it via `take_records_from`
/// and surfaces it on that frame's `FrameResult`.
pub(crate) fn push_frame_record(record: FrameAllocRecord) {
    FRAME_RECORDS.with(|r| r.borrow_mut().push(record));
}

/// Current length of the thread's record log; snapshot at the start of
/// a frame window to bound the window's claim.
pub(crate) fn records_len() -> usize {
    FRAME_RECORDS.with(|r| r.borrow().len())
}

/// Claim (remove and return) the records pushed since `start`, in
/// invocation-completion order. Frame windows nest strictly on one
/// thread and inner windows claim their tail first, so `start` is
/// always within bounds for the enclosing window.
pub(crate) fn take_records_from(start: usize) -> Vec<FrameAllocRecord> {
    FRAME_RECORDS.with(|r| {
        let mut records = r.borrow_mut();
        let start = start.min(records.len());
        records.split_off(start)
    })
}

/// Counting wrapper around the system allocator. Install it in a test
/// binary with [`crate::install_counting_allocator!`]; frame stats stay
/// zero without it.
pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_alloc(layout.size(), layout.align());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    // `realloc` and `alloc_zeroed` intentionally not overridden: the
    // default impls route through `alloc`, charging a realloc as a
    // fresh allocation — the same thing the rv32 bump allocator does.
}

/// Install the counting allocator as the binary's `#[global_allocator]`.
/// Invoke once at the top level of a test binary (each `tests/*.rs` file
/// is its own binary) to make frame allocation stats available.
#[macro_export]
macro_rules! install_counting_allocator {
    () => {
        #[global_allocator]
        static GROUNDHOG_NATIVE_EXEC_COUNTING_ALLOCATOR:
            $crate::alloc_stats::CountingAllocator =
            $crate::alloc_stats::CountingAllocator;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta<R>(f: impl FnOnce() -> R) -> (R, AllocStats) {
        let start = totals();
        let r = f();
        let end = totals();
        (
            r,
            AllocStats {
                allocs: end.allocs - start.allocs,
                bytes_total: end.bytes_total - start.bytes_total,
                heap_used: end.heap_used - start.heap_used,
            },
        )
    }

    #[test]
    fn record_alloc_accumulates_counts_and_bytes() {
        let ((), d) = delta(|| {
            record_alloc(10, 1);
            record_alloc(30, 1);
        });
        assert_eq!(d.allocs, 2);
        assert_eq!(d.bytes_total, 40);
        assert_eq!(d.heap_used, 40);
    }

    #[test]
    fn heap_used_includes_bump_alignment_padding() {
        // Replay of the rv32 bump allocator: after a 10-byte alloc the
        // cursor sits at offset 10; an 8-aligned alloc pads to 16 first.
        let ((), d) = delta(|| {
            record_alloc(10, 1);
            record_alloc(8, 8);
        });
        assert_eq!(d.bytes_total, 18);
        assert_eq!(d.heap_used, d.bytes_total + 6);
    }

    #[test]
    fn scope_reports_exclusive_stats_for_nested_frames() {
        let outer = FrameMeasureScope::open();
        record_alloc(100, 1);

        let inner = FrameMeasureScope::open();
        record_alloc(40, 1);
        let inner_stats = inner.close();

        record_alloc(10, 1);
        let outer_stats = outer.close();

        assert_eq!(inner_stats.allocs, 1);
        assert_eq!(inner_stats.bytes_total, 40);
        assert_eq!(outer_stats.allocs, 2);
        assert_eq!(outer_stats.bytes_total, 110);
    }

    #[test]
    fn grandchild_frames_are_not_double_subtracted() {
        let outer = FrameMeasureScope::open();
        record_alloc(1, 1);

        let child = FrameMeasureScope::open();
        record_alloc(2, 1);

        let grandchild = FrameMeasureScope::open();
        record_alloc(4, 1);
        let gc_stats = grandchild.close();

        let child_stats = child.close();
        let outer_stats = outer.close();

        assert_eq!(gc_stats.bytes_total, 4);
        assert_eq!(child_stats.bytes_total, 2);
        assert_eq!(child_stats.allocs, 1);
        assert_eq!(outer_stats.bytes_total, 1);
        assert_eq!(outer_stats.allocs, 1);
    }

    #[test]
    fn sibling_child_frames_all_subtract_from_parent() {
        let parent = FrameMeasureScope::open();

        let c1 = FrameMeasureScope::open();
        record_alloc(10, 1);
        assert_eq!(c1.close().bytes_total, 10);

        let c2 = FrameMeasureScope::open();
        record_alloc(20, 1);
        assert_eq!(c2.close().bytes_total, 20);

        record_alloc(3, 1);
        let parent_stats = parent.close();
        assert_eq!(parent_stats.bytes_total, 3);
        assert_eq!(parent_stats.allocs, 1);
    }

    fn record(index: i32) -> FrameAllocRecord {
        FrameAllocRecord {
            mock_contract_index: index,
            entrypoint: ContractEntrypointID::from_u64(0x1122_3344),
            stats: AllocStats::ZERO,
        }
    }

    #[test]
    fn take_records_from_claims_only_the_tail_window() {
        // Nested frame windows claim their own tail of the log; the
        // enclosing window then claims what remains.
        let outer_start = records_len();
        push_frame_record(record(1));
        push_frame_record(record(2));

        let inner_start = records_len();
        push_frame_record(record(3));
        let inner = take_records_from(inner_start);
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0].mock_contract_index, 3);

        let outer = take_records_from(outer_start);
        assert_eq!(outer.len(), 2);
        assert_eq!(outer[0].mock_contract_index, 1);
        assert_eq!(outer[1].mock_contract_index, 2);

        assert_eq!(records_len(), outer_start);
    }
}
