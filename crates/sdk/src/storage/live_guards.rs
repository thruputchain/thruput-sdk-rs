//! Native-test-only tracking of live deferred-write guards (`ValueMut`).
//!
//! On a real guest a terminal Return syscall skips `Drop`, silently
//! discarding a live guard's flush; the native mock unwinds, so `Drop`
//! runs and tests can't see the loss. This counter inverts that: the
//! terminal-return funnel panics on a live guard, so any native test
//! exercising the path fails loudly. The entire module (and every call
//! site) is compiled only for the native test environment — guest
//! artifacts contain none of it.

use core::cell::Cell;

std::thread_local! {
    static LIVE_VALUE_MUTS: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn guard_created() {
    LIVE_VALUE_MUTS.with(|c| c.set(c.get() + 1));
}

pub(crate) fn guard_dropped() {
    LIVE_VALUE_MUTS.with(|c| c.set(c.get().saturating_sub(1)));
}

/// Called by `contract_return` before the Return syscall.
///
/// Fails the invocation as `Unrecoverable` rather than panicking plainly: a
/// plain panic is rewritten by the mock's panic hook into a `Deterministic`
/// error — indistinguishable from a legitimate contract abort, which a test
/// calling the method via `try_*` would catch and carry on past. A dropped
/// storage write is a contract bug, not an abort, so it must not be
/// catchable.
pub(crate) fn assert_no_live_guards() {
    let n = LIVE_VALUE_MUTS.with(|c| c.get());
    if n == 0 {
        return;
    }
    let msg = std::format!(
        "contract_return with {n} live ValueMut guard(s): on a real guest \
         the terminal Return syscall skips Drop, silently discarding their \
         storage writes. Drop the guard(s) before returning."
    );
    #[cfg(feature = "mock")]
    groundhog_native_exec::invoke_error::unrecoverable_panic(&msg);
    #[cfg(not(feature = "mock"))]
    panic!("{msg}");
}

/// Zeroes the counter for the duration of a nested in-process mock frame
/// (the callee has its own memory on a real guest, so the caller's live
/// guards must not leak into its check); restores the saved count on drop,
/// including on unwind.
pub(crate) struct FrameGuardReset(u32);

impl FrameGuardReset {
    pub(crate) fn new() -> Self {
        Self(LIVE_VALUE_MUTS.with(|c| c.replace(0)))
    }
}

impl Drop for FrameGuardReset {
    fn drop(&mut self) {
        let saved = self.0;
        LIVE_VALUE_MUTS.with(|c| c.set(saved));
    }
}
