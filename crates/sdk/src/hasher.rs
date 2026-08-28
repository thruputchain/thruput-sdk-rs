use crate::SHA256Hash;
use crate::exn::syscall_throw;
use crate::sys::{SyscallErrorCode, SyscallNo};
use core::fmt::*;

/// The maximum number of hashers that can be in existence at once
pub const N_HASHERS: usize = 8;

type HasherIndex = u32;

/// An incremental hasher, implemented by the VM.
///
/// A value of this type is meaningful only in the current contract invocation,
/// so it can't be stored.
pub struct Hasher {
    index: HasherIndex,
    needs_reset: bool,
}

impl Hasher {
    /// Get a hasher.
    ///
    /// Throws a syscall exception (`kSHA256Update` /
    /// `kSyscallErrorCodeInvalidSyscallInput` — the error the VM answers
    /// stream-cap exhaustion with) if more than [`N_HASHERS`] hashers are
    /// currently in use.
    pub fn new() -> Hasher {
        Hasher::try_new().unwrap_or_else(|| {
            syscall_throw(
                SyscallNo::kSHA256Update,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
            )
        })
    }

    /// Get a hasher if one is available.
    pub fn try_new() -> Option<Hasher> {
        slot_alloc().map(Self::new_at)
    }

    fn new_at(index: HasherIndex) -> Hasher {
        Hasher {
            index,
            needs_reset: true,
        }
    }

    /// Append data to the hasher input.
    ///
    /// Throws a syscall exception (aborting the frame) if the host call
    /// fails.
    pub fn update<I: AsRef<[u8]>>(&mut self, input: &I) {
        let index = self.index;
        let reset = self.needs_reset;

        self.needs_reset = false;

        crate::sys::raw_SHA256Update(index, input.as_ref(), reset)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Update, e));
    }

    /// Fork the hasher: a new hasher whose state is an exact copy of this
    /// one's, including all input absorbed so far. The two diverge
    /// independently afterwards.
    ///
    /// Copying is constant-time in the VM regardless of how much input has
    /// been absorbed, so forking is the cheap way to hash many inputs that
    /// share a prefix: absorb the prefix once, then fork per suffix.
    ///
    /// Throws a syscall exception (aborting the frame) if the host call
    /// fails, or (`kSHA256Copy` / `kSyscallErrorCodeInvalidSyscallInput` —
    /// the error the VM answers stream-cap exhaustion with) if more than
    /// [`N_HASHERS`] hashers are currently in use.
    pub fn fork(&self) -> Hasher {
        self.try_fork().unwrap_or_else(|| {
            syscall_throw(
                SyscallNo::kSHA256Copy,
                SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput.into(),
            )
        })
    }

    /// Fork the hasher if a slot is available (see [`fork`](Self::fork)).
    pub fn try_fork(&self) -> Option<Hasher> {
        let slot = slot_alloc()?;
        if self.needs_reset {
            // No VM-side stream exists for this hasher yet (nothing absorbed
            // since construction), so the fork is equally fresh: it creates
            // its own stream on first update. No syscall needed.
            return Some(Hasher::new_at(slot));
        }
        crate::sys::raw_SHA256Copy(self.index, slot)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Copy, e));
        Some(Hasher {
            index: slot,
            needs_reset: false,
        })
    }

    /// Crate-internal: replace this hasher's stream with a fresh one holding
    /// exactly `bytes`. Used by the storage layer's streamed key derivation
    /// to rebuild its cached prefix midstate in place.
    pub(crate) fn reabsorb(&mut self, bytes: &[u8]) {
        self.needs_reset = false;
        crate::sys::raw_SHA256Update(self.index, bytes, true)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Update, e));
    }

    /// Crate-internal: overwrite this hasher's state with a copy of `src`'s
    /// stream, reporting failure instead of throwing. `false` means `src`'s
    /// VM-side stream does not exist — for the storage layer's cached prefix
    /// midstate that signals a stale cache (a new invocation reset the VM's
    /// streams but not the contract-local cache), which the caller recovers
    /// from by rebuilding the source. `self` is unchanged on failure.
    pub(crate) fn copy_from(&mut self, src: &Hasher) -> bool {
        if src.needs_reset {
            return false;
        }
        let ok = crate::sys::raw_SHA256Copy(src.index, self.index).is_ok();
        if ok {
            self.needs_reset = false;
        }
        ok
    }

    /// Consume the hasher and return the SHA256 hash of the prior input.
    ///
    /// Throws a syscall exception (aborting the frame) if a host call fails.
    pub fn finish(mut self) -> SHA256Hash {
        let index = self.index;

        // If the hasher was never updated, the VM has not created a stream at
        // this index yet. Issue an empty reset update so that SHA256Finalize
        // sees a fresh, empty stream and returns the hash of the empty input.
        // Mirrors how `update()` invokes the syscall.
        if self.needs_reset {
            self.needs_reset = false;
            crate::sys::raw_SHA256Update(index, &[], true).unwrap_or_else(
                |e| syscall_throw(SyscallNo::kSHA256Update, e),
            );
        }

        let mut output = core::mem::MaybeUninit::uninit();

        crate::sys::uninit_SHA256Finalize(index, &mut output)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Finalize, e));

        // Finalize erased the VM-side stream; mark it gone so the drop below
        // doesn't issue a second finalize for it.
        self.needs_reset = true;

        // SAFETY: The syscall succeeded, so the output has been initialized
        unsafe { output.assume_init() }
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Hasher::new()
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // Indices are never reused, so nothing later reclaims this hasher's
        // VM-side stream; discard it via SHA256Finalize so the capacity the
        // allocator reserves stays available to future hashers. A hasher
        // that never absorbed input has no stream, and `finish` marks the
        // stream gone after its own finalize; both skip the syscall. The
        // result is deliberately ignored: the stream may legitimately be
        // gone already (a stale cached hasher from a previous invocation on
        // native hosts).
        if !self.needs_reset && !unwinding() {
            let mut discard = core::mem::MaybeUninit::uninit();
            let _ =
                crate::sys::uninit_SHA256Finalize(self.index, &mut discard);
        }
        slot_release();
    }
}

/// True while an unwind is in progress on a native host. The drop-time
/// finalize is skipped then: the frame is being torn down and its streams
/// die with it, and a syscall answered with an unrecoverable host state
/// would turn the unwind into a double panic (an immediate process abort).
/// rv32 guests do not unwind — a contract panic aborts the frame — so
/// there drops never run mid-panic.
#[cfg(all(not(any(doc, target_arch = "riscv32")), feature = "std"))]
fn unwinding() -> bool {
    std::thread::panicking()
}

#[cfg(any(doc, target_arch = "riscv32", not(feature = "std")))]
fn unwinding() -> bool {
    false
}

impl Debug for Hasher {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "Hasher_{}", self.index)
    }
}

////////////////////////////////////////////////////////////
#[allow(unused)]
use crate::cell::ContractCell;
use crate::contract_local;

// Bookkeeping for the VM's per-invocation incremental-hash streams: the VM
// caps how many streams are live at once (kMaxSHA256Buffers) but keys them
// by arbitrary u32 index and erases one on SHA256Finalize. Allocation is
// therefore a capacity reservation plus a bump index — indices are never
// reused, and freeing the stream itself is `Drop`/`finish`'s job.
contract_local! {
    static LIVE: ContractCell<u32> = const { ContractCell::new(0) };
    static NEXT_INDEX: ContractCell<u32> = const { ContractCell::new(0) };
}

// TODO: rewrite slot_alloc and slot_release to use .update() once
// local_key_cell_update is stabilized (rust#143989). Using get/set for now
// because .update() on LocalKey<Cell<T>> requires nightly, which breaks
// doc builds on stable (e.g. kellnr).
#[cfg(any(doc, feature = "std", target_arch = "riscv32"))]
fn slot_alloc() -> Option<HasherIndex> {
    let live = LIVE.get();
    if live as usize >= N_HASHERS {
        return None;
    }
    LIVE.set(live + 1);
    let index = NEXT_INDEX.get();
    NEXT_INDEX.set(index.wrapping_add(1));
    Some(index)
}

// Suppress compile errors when building for native without std
#[cfg(not(any(doc, feature = "std", target_arch = "riscv32")))]
fn slot_alloc() -> Option<HasherIndex> {
    None
}

#[cfg(any(doc, feature = "std", target_arch = "riscv32"))]
fn slot_release() {
    // An already-zero count indicates an internal bookkeeping bug, but
    // panicking here would turn an unwind already in progress into a
    // double panic (an immediate process abort), so tolerate it.
    LIVE.set(LIVE.get().saturating_sub(1));
}

#[cfg(not(any(doc, feature = "std", target_arch = "riscv32")))]
fn slot_release() {}

// Regression coverage for the drop-during-unwind double panic.
//
// A panic from a destructor during an unwind is a double panic: the
// process aborts immediately, killing the whole test binary instead of
// failing one test. Drop runs almost exclusively during unwinds (a throw
// or panic with a live hasher on the stack), so its bookkeeping must
// tolerate a desynced reservation count rather than panic on it.
//
// The desynced state is not reachable through the public API on a single
// thread (only `drop` releases a reservation), so reconstruct it directly
// with the module's own `slot_release`, the same way `test_slots` drives
// the allocator. With a panicking release this test would abort the
// entire test process; with the tolerant release the original panic is
// the only panic and unwinds normally.
#[test]
fn test_drop_of_desynced_slot_during_unwind_is_not_a_double_panic() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let hasher = Hasher::try_new().expect("a hasher slot must be available");

    // Desync the bookkeeping: release the live hasher's reservation behind
    // its back, so its drop underflows the count.
    slot_release();

    let unwind = catch_unwind(AssertUnwindSafe(move || {
        // Move the hasher in so the unwind (not normal scope exit) drops it.
        let _live = hasher;
        panic!("frame failure with a live hasher");
    }));

    let payload = unwind.expect_err("the closure must panic");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"frame failure with a live hasher"),
        "the original panic must be the only panic"
    );
}

#[test]
fn test_slots() {
    let mut last = slot_alloc().expect("No slots available");

    for _ in 1..N_HASHERS {
        let slot = slot_alloc().expect("No slots available");
        assert!(slot > last, "indices must increase monotonically");
        last = slot;
    }

    assert!(slot_alloc().is_none(), "capacity must be exhausted");

    slot_release();
    let reissued =
        slot_alloc().expect("released capacity must admit a new hasher");
    assert!(reissued > last, "released capacity must not reuse an index");
}
