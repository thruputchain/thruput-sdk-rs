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
//! relative comparisons and regression bounds — not exact budget math.
//! Harness bookkeeping (frame open/close, syscall implementations,
//! result construction) is excluded via the internal `PauseGuard`, but:
//!
//! - Native `usize`/pointers are 8 bytes; rv32's are 4.
//! - The measured window still includes allocations made by harness
//!   code running *as* the frame's closure (e.g. `FrameBuilder::run`'s
//!   XDR-encode of the return value, which on the guest uses the
//!   no-alloc scratch buffer instead).
//! - A `panic!` raised directly in contract code allocates its payload
//!   box natively inside the measured window (panics raised by syscalls
//!   are excluded along with the syscall).
//! - A frame spanning several `execute` steps (`FrameAction::Continue`)
//!   replays each step from a fresh cursor, so padding at step
//!   boundaries is not charged; the real guest keeps one heap across
//!   the whole frame.

use groundhog_xdr::contract::{Contract, ContractEntrypointID};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};

/// Allocation counters for one invocation frame (exclusive of callees).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllocStats {
    /// Number of allocation calls (including the alloc half of reallocs).
    pub allocs: u64,
    /// Cumulative bytes requested. Never decremented — the rv32 bump
    /// allocator never reuses freed memory, so cumulative (not live) is
    /// what the guest pays. Padding-free, so it maps exactly to known
    /// request sizes; use [`Self::heap_used`] for the guest-faithful
    /// consumption figure.
    pub bytes_total: u64,
    /// Simulated rv32 bump-allocator consumption: the frame's own
    /// allocation sequence replayed through the guest allocator's
    /// cursor arithmetic (align-up, then advance by size) on a fresh
    /// heap, so alignment padding is included. This is the field to
    /// bound when checking a heap budget.
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
    contract: Contract,
    /// Entrypoint that was invoked.
    pub entrypoint: ContractEntrypointID,
    /// Exclusive allocation stats for the frame.
    pub stats: AllocStats,
}

impl FrameAllocRecord {
    pub(crate) fn new(
        contract: Contract,
        entrypoint: ContractEntrypointID,
        stats: AllocStats,
    ) -> Self {
        Self {
            contract,
            entrypoint,
            stats,
        }
    }

    /// The mock contract whose method this frame invoked; compare
    /// against [`crate::mock_contract::MockContract::as_contract`].
    pub fn contract(&self) -> &Contract {
        &self.contract
    }
}

// The thread's measurement context: the counters of the *innermost
// open frame window* (or of a root pseudo-window when none is open).
// `FrameMeasureScope::open` saves this struct and zeroes it; `close`
// reads it as the frame's exclusive stats and restores the save — the
// window "stack" lives implicitly in the nested scope values on the
// Rust call stack, so a frame never observes its callees' allocations
// at all.
//
// Const-initialized `Cell`s: lazy TLS initialization can allocate,
// which would recurse into the counting allocator, and none of these
// have destructors so nothing is registered at first touch either.
struct MeasureState {
    allocs: Cell<u64>,
    bytes_total: Cell<u64>,
    // Bump-cursor replay of the guest allocator for the current window,
    // starting from 0 at window open — a fresh guest heap, exactly like
    // each frame's own instance on the real target.
    heap_cursor: Cell<u64>,
    // Number of open FrameMeasureScopes on this thread. Zero means a
    // cross-call record pushed now would have no owning frame.
    depth: Cell<u32>,
    // While set, allocations are harness bookkeeping and not counted
    // (see `PauseGuard`). Window opens clear it for contract code.
    paused: Cell<bool>,
}

thread_local! {
    static STATE: MeasureState = const {
        MeasureState {
            allocs: Cell::new(0),
            bytes_total: Cell::new(0),
            heap_cursor: Cell::new(0),
            depth: Cell::new(0),
            paused: Cell::new(false),
        }
    };
    static FRAME_RECORDS: RefCell<Vec<FrameAllocRecord>> =
        const { RefCell::new(Vec::new()) };
}

/// RAII guard marking a harness-bookkeeping region: allocations made
/// while it lives are excluded from frame stats (frame open/close, the
/// mock syscall implementations, record bookkeeping, result
/// construction). [`FrameMeasureScope::open`] re-enables counting, so
/// contract code re-entered *through* a paused region — e.g. the
/// `Invoke` syscall trampolining into a mock method — still measures.
pub(crate) struct PauseGuard {
    prev: bool,
}

impl PauseGuard {
    pub(crate) fn new() -> Self {
        Self {
            prev: STATE.with(|s| s.paused.replace(true)),
        }
    }
}

impl Drop for PauseGuard {
    fn drop(&mut self) {
        STATE.with(|s| s.paused.set(self.prev));
    }
}

/// Record one allocation of `size` bytes with `align` alignment into
/// the innermost open window's counters. Called by the counting
/// allocator; exposed to unit tests for deterministic sequences.
pub(crate) fn record_alloc(size: usize, align: usize) {
    STATE.with(|s| {
        if s.paused.get() {
            return;
        }
        s.allocs.set(s.allocs.get() + 1);
        s.bytes_total.set(s.bytes_total.get() + size as u64);
        // The rv32 bump allocator's arithmetic: align the cursor up,
        // then advance by the request size.
        let aligned = s.heap_cursor.get().next_multiple_of(align as u64);
        s.heap_cursor.set(aligned + size as u64);
    });
}

/// Snapshot of the innermost open window's counters (or the root
/// pseudo-window's, when no scope is open). Production code reads
/// counters only through `FrameMeasureScope`; this is for unit tests.
#[cfg(test)]
fn current_counters() -> AllocStats {
    STATE.with(|s| AllocStats {
        allocs: s.allocs.get(),
        bytes_total: s.bytes_total.get(),
        heap_used: s.heap_cursor.get(),
    })
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
/// code, `close` after; `close` returns the frame's *exclusive* stats.
///
/// `open` saves the enclosing window's counters and zeroes them; `close`
/// reads the zeroed-then-accumulated counters as this frame's stats and
/// restores the save. A frame therefore never observes its callees'
/// allocations at all, and its `heap_used` is an exact replay of its own
/// allocation sequence on a fresh guest heap — matching the real system,
/// where each frame is a separate guest instance.
pub(crate) struct FrameMeasureScope {
    saved: AllocStats,
    prev_paused: bool,
}

impl FrameMeasureScope {
    pub(crate) fn open() -> Self {
        STATE.with(|s| Self {
            saved: AllocStats {
                allocs: s.allocs.replace(0),
                bytes_total: s.bytes_total.replace(0),
                heap_used: s.heap_cursor.replace(0),
            },
            // Contract code counts, even when the frame was entered
            // through a paused harness region (Invoke -> trampoline).
            prev_paused: {
                s.depth.set(s.depth.get() + 1);
                s.paused.replace(false)
            },
        })
    }

    pub(crate) fn close(self) -> AllocStats {
        STATE.with(|s| {
            let exclusive = AllocStats {
                allocs: s.allocs.replace(self.saved.allocs),
                bytes_total: s.bytes_total.replace(self.saved.bytes_total),
                heap_used: s.heap_cursor.replace(self.saved.heap_used),
            };
            s.paused.set(self.prev_paused);
            s.depth.set(s.depth.get() - 1);
            exclusive
        })
    }
}

/// Append a cross-call frame record to the current thread's log. The
/// enclosing frame's `execute` window claims it via `take_records_from`
/// and surfaces it on that frame's `FrameResult`.
///
/// With no window open — a mock method dispatched outside the mock-frame
/// harness (e.g. `TransactionHandler::root_invoke`, or a wrapped closure
/// invoked directly) — the record has no owning frame: drop it rather
/// than strand it in the log, where it would leak and could later be
/// claimed by an unrelated frame's window.
pub(crate) fn push_frame_record(record: FrameAllocRecord) {
    if STATE.with(|s| s.depth.get()) == 0 {
        return;
    }
    let _pause = PauseGuard::new();
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
    let _pause = PauseGuard::new();
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
        let start = current_counters();
        let r = f();
        (r, current_counters() - start)
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

    fn kmock(index: i32) -> Contract {
        Contract::kMockTest {
            mock_contract_id: index,
        }
    }

    fn record(index: i32) -> FrameAllocRecord {
        FrameAllocRecord::new(
            kmock(index),
            ContractEntrypointID::from_u64(0x1122_3344),
            AllocStats::ZERO,
        )
    }

    #[test]
    fn scope_heap_used_replays_a_fresh_guest_heap() {
        // Leave the thread cursor misaligned before any window opens.
        record_alloc(3, 1);

        let outer = FrameMeasureScope::open();
        record_alloc(8, 8); // fresh cursor: no padding

        let inner = FrameMeasureScope::open();
        record_alloc(10, 1);
        record_alloc(8, 8); // 10 -> pad to 16 -> 24
        let inner_stats = inner.close();

        record_alloc(4, 4); // outer resumes at 8: aligned -> 12
        let outer_stats = outer.close();

        assert_eq!(
            inner_stats.heap_used, 24,
            "child replays its own sequence from a fresh heap"
        );
        assert_eq!(
            outer_stats.heap_used, 12,
            "parent's replay is unaffected by the child window \
             and by pre-window cursor state"
        );
    }

    #[test]
    fn records_without_an_open_frame_window_are_dropped() {
        let base = records_len();
        push_frame_record(record(1));
        assert_eq!(
            records_len(),
            base,
            "no frame window is open, so the record has no owner and \
             must be dropped instead of stranded"
        );

        let scope = FrameMeasureScope::open();
        push_frame_record(record(2));
        assert_eq!(records_len(), base + 1);
        scope.close();
    }

    #[test]
    fn take_records_from_claims_only_the_tail_window() {
        // Nested frame windows claim their own tail of the log; the
        // enclosing window then claims what remains. (A window must be
        // open for pushes to be accepted at all.)
        let window = FrameMeasureScope::open();
        let outer_start = records_len();
        push_frame_record(record(1));
        push_frame_record(record(2));

        let inner_start = records_len();
        push_frame_record(record(3));
        let inner = take_records_from(inner_start);
        assert_eq!(inner.len(), 1);
        assert_eq!(*inner[0].contract(), kmock(3));

        let outer = take_records_from(outer_start);
        assert_eq!(outer.len(), 2);
        assert_eq!(*outer[0].contract(), kmock(1));
        assert_eq!(*outer[1].contract(), kmock(2));

        assert_eq!(records_len(), outer_start);
        window.close();
    }
}
