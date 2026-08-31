//! Native-test-only tracking of live deferred-write guards (`ValueMut`,
//! `CapacityConstraint`).
//!
//! On a real guest a terminal Return syscall skips `Drop`, silently
//! discarding a live guard's flush; the native mock unwinds, so `Drop`
//! runs and tests can't see the loss. This counter inverts that: the
//! terminal-return funnel fails on a live guard, so any native test
//! exercising the path fails loudly.
//!
//! The guest build gets the no-op stubs in `storage/mod.rs` instead of this
//! module, so call sites need no `cfg` and guest artifacts contain none of
//! this. (Same dual-impl convention as [`crate::rt::trap`].)

use core::cell::Cell;

std::thread_local! {
    static LIVE_VALUE_MUTS: Cell<u32> = const { Cell::new(0) };
}

/// RAII registration token, held as a field by every deferred-write storage
/// guard (today [`ValueMut`](super::ValueMut) and
/// [`CapacityConstraint`](super::CapacityConstraint)).
///
/// Holding the field *is* the registration: the count is maintained by the
/// type system rather than by paired `guard_created`/`guard_dropped` calls a
/// future guard author has to remember to write. `mem::forget`ing a guard
/// correctly leaves it counted as live.
pub(crate) struct LiveGuard;

impl LiveGuard {
    pub(crate) fn new() -> Self {
        LIVE_VALUE_MUTS.with(|c| c.set(c.get() + 1));
        Self
    }
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        // Cannot underflow: the only way to obtain a `LiveGuard` is `new`,
        // which incremented.
        LIVE_VALUE_MUTS.with(|c| c.set(c.get() - 1));
    }
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
    std::eprintln!(
        "contract_return with {n} live deferred-write storage guard(s): on \
         a real guest the terminal Return syscall skips Drop, silently \
         discarding their storage writes. Drop the guard(s) before \
         returning."
    );
    #[cfg(feature = "dev")]
    std::panic::panic_any(
        groundhog_native_exec::invoke_error::unrecoverable_error(),
    );
    #[cfg(not(feature = "dev"))]
    panic!("contract_return with {n} live deferred-write storage guard(s)");
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
