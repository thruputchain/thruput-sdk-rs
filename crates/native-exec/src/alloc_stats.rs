//! Native allocation estimates for contract tests.
//!
//! Install [`crate::install_counting_allocator!`] once per test binary.
//! Measurements count each frame's own allocations, excluding callees and
//! host syscalls. Deallocation does not reduce the counters, matching the
//! guest bump allocator's cumulative consumption.
//!
//! These are regression estimates, not RV32 memory requirements: native
//! pointer sizes and code paths differ, frame closures can include test
//! allocations, and panic diagnostics allocate. The simulated heap starts
//! at offset zero; it does not model the linked guest heap's base alignment.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thruput_xdr::contract::{Contract, ContractEntrypointID};

/// Cumulative native allocations for one frame, exclusive of callees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocStats {
    /// Number of allocations, including fresh allocations for reallocations.
    pub allocs: u64,
    /// Total requested bytes; freeing memory does not subtract from this.
    pub bytes_total: u64,
    /// Native allocation sizes replayed through an aligned bump cursor.
    /// This is an estimate, not an exact or conservative RV32 heap bound.
    pub estimated_heap_bytes: u64,
}

/// One completed native mock body, including bodies that fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameAllocRecord {
    /// The invoked mock contract.
    pub contract: Contract,
    /// The requested method, including undefined methods handled by a fallback.
    pub entrypoint: ContractEntrypointID,
    /// The invocation's own allocation estimate.
    pub stats: AllocStats,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Default)]
struct Measurement {
    stats: Option<AllocStats>,
    paused: bool,
}

thread_local! {
    static CURRENT: Cell<Measurement> = const { Cell::new(Measurement {
        stats: None, paused: false,
    }) };
}

pub(crate) struct PauseGuard(bool, PhantomData<Rc<()>>);

impl PauseGuard {
    pub(crate) fn new() -> Self {
        Self(
            CURRENT.with(|c| {
                let mut state = c.get();
                let previous = state.paused;
                state.paused = true;
                c.set(state);
                previous
            }),
            PhantomData,
        )
    }
}

impl Drop for PauseGuard {
    fn drop(&mut self) {
        CURRENT.with(|c| {
            let mut state = c.get();
            state.paused = self.0;
            c.set(state);
        });
    }
}

pub(crate) struct FrameMeasureScope(Measurement, PhantomData<Rc<()>>);

impl FrameMeasureScope {
    pub(crate) fn open(previous: Option<AllocStats>) -> Self {
        let stats = INSTALLED
            .load(Ordering::Relaxed)
            .then(|| previous.unwrap_or_default());
        Self(
            CURRENT.with(|c| {
                c.replace(Measurement {
                    stats,
                    paused: false,
                })
            }),
            PhantomData,
        )
    }

    pub(crate) fn close(self) -> Option<AllocStats> {
        CURRENT.with(|c| c.get().stats)
    }
}

impl Drop for FrameMeasureScope {
    fn drop(&mut self) {
        CURRENT.with(|c| c.set(self.0));
    }
}

#[derive(Default)]
struct Records {
    windows: usize,
    entries: Vec<FrameAllocRecord>,
}

// Explicit execute windows nest or run sequentially within an Rc-owned
// environment. Callbacks can run on other workers, so only the completed
// records cross threads; allocation counters remain local to each body.
#[derive(Clone, Default)]
pub(crate) struct RecordLog(Arc<Mutex<Records>>);

impl RecordLog {
    pub(crate) fn open(&self) -> Option<RecordWindow> {
        if !INSTALLED.load(Ordering::Relaxed) {
            return None;
        }
        let _pause = PauseGuard::new();
        let mut records = self.0.lock().unwrap();
        records.windows += 1;
        Some(RecordWindow {
            log: self.clone(),
            start: records.entries.len(),
            closed: false,
        })
    }

    pub(crate) fn push(&self, record: FrameAllocRecord) {
        let _pause = PauseGuard::new();
        let mut records = self.0.lock().unwrap();
        if records.windows != 0 {
            records.entries.push(record);
        }
    }
}

pub(crate) struct RecordWindow {
    log: RecordLog,
    start: usize,
    closed: bool,
}

impl RecordWindow {
    pub(crate) fn finish(mut self) -> Vec<FrameAllocRecord> {
        let _pause = PauseGuard::new();
        let mut records = self.log.0.lock().unwrap();
        let entries = records.entries.split_off(self.start);
        records.windows -= 1;
        self.closed = true;
        entries
    }
}

impl Drop for RecordWindow {
    fn drop(&mut self) {
        if !self.closed {
            let _pause = PauseGuard::new();
            let mut records = self.log.0.lock().unwrap();
            records.entries.truncate(self.start);
            records.windows -= 1;
        }
    }
}

/// System allocator wrapper used by [`crate::install_counting_allocator!`].
/// Outside a measured frame it delegates without collecting statistics.
#[derive(Debug)]
pub struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        INSTALLED.store(true, Ordering::Relaxed);
        // try_with also permits allocations during thread-local teardown.
        let _ = CURRENT.try_with(|c| {
            let mut state = c.get();
            if !state.paused
                && let Some(stats) = &mut state.stats
            {
                stats.allocs = stats.allocs.saturating_add(1);
                stats.bytes_total =
                    stats.bytes_total.saturating_add(layout.size() as u64);
                let mask = layout.align() as u64 - 1;
                let aligned =
                    stats.estimated_heap_bytes.saturating_add(mask) & !mask;
                stats.estimated_heap_bytes =
                    aligned.saturating_add(layout.size() as u64);
                c.set(state);
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    // Defaults allocate afresh for realloc/alloc_zeroed, as the guest does.
}

/// Install native allocation estimates for this test binary.
/// Do not combine with another global allocator.
#[macro_export]
macro_rules! install_counting_allocator {
    () => {
        #[global_allocator]
        static THRUPUT_COUNTING_ALLOCATOR:
            $crate::alloc_stats::CountingAllocator =
            $crate::alloc_stats::CountingAllocator;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::install_counting_allocator!();

    fn allocate(size: usize, align: usize) {
        let layout = Layout::from_size_align(size, align).unwrap();
        unsafe {
            let ptr = std::alloc::alloc(layout);
            assert!(!ptr.is_null());
            std::alloc::dealloc(ptr, layout);
        }
    }

    fn record(stats: AllocStats) -> FrameAllocRecord {
        FrameAllocRecord {
            contract: Contract::kMockTest {
                mock_contract_id: 1,
            },
            entrypoint: ContractEntrypointID::from_u64(1),
            stats,
        }
    }

    #[test]
    fn continuation_keeps_padding_and_children_are_exclusive() {
        let first = FrameMeasureScope::open(None);
        allocate(1, 1);
        let saved = first.close();
        let second = FrameMeasureScope::open(saved);
        let child = FrameMeasureScope::open(None);
        allocate(1024, 8);
        let child = child.close().unwrap();
        allocate(8, 8);
        let parent = second.close().unwrap();
        assert_eq!(child.bytes_total, 1024);
        assert_eq!(parent.bytes_total, 9);
        assert_eq!(parent.estimated_heap_bytes, 16);
    }

    #[test]
    fn scopes_restore_after_unwind() {
        let parent = FrameMeasureScope::open(None);
        let caught = std::panic::catch_unwind(|| {
            let _child = FrameMeasureScope::open(None);
            allocate(1024, 8);
            panic!("child failed");
        });
        // Account for this allocation only; child panic bookkeeping is excluded.
        allocate(8, 8);
        let parent = parent.close().unwrap();
        assert!(caught.is_err());
        assert_eq!(parent.bytes_total, 8);
    }

    #[test]
    fn worker_records_belong_to_the_environment_window() {
        let log = RecordLog::default();
        let parent = log.open().unwrap();
        let worker_log = log.clone();
        std::thread::spawn(move || {
            let scope = FrameMeasureScope::open(None);
            allocate(4096, 8);
            let stats = scope.close().unwrap();
            worker_log.push(record(stats));
        })
        .join()
        .unwrap();
        let records = parent.finish();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].stats.bytes_total, 4096);
    }

    #[test]
    fn record_windows_claim_only_their_calls_and_clean_up_on_unwind() {
        let log = RecordLog::default();
        let record = record(AllocStats::default());
        log.push(record.clone()); // No owner: must not leak into the next window.
        let parent = log.open().unwrap();
        log.push(record.clone());
        let child = log.open().unwrap();
        log.push(record.clone());
        assert_eq!(child.finish().len(), 1);
        let _ = std::panic::catch_unwind(|| {
            let _abandoned = log.open().unwrap();
            log.push(record.clone());
            panic!("abandoned window");
        });
        assert_eq!(parent.finish().len(), 1);
        assert!(log.open().unwrap().finish().is_empty());
    }
}
