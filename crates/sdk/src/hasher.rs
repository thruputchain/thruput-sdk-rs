use crate::SHA256Hash;
use crate::exn::syscall_throw;
use crate::sys::SyscallNo;
use core::fmt::*;

/// The maximum number of hashers that can be in existence at once
pub const N_HASHERS: usize = 8;

type HasherIndex = u8;

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
    /// Panics if more than [`N_HASHERS`] hashers are currently in use.
    ///
    pub fn new() -> Hasher {
        Hasher::try_new().unwrap_or_else(|| {
            panic!("Hasher::new: no more hashers are available");
        })
    }

    /// Get a hasher if one is available.
    pub fn try_new() -> Option<Hasher> {
        slot_alloc().map(Self::new_at)
    }

    fn new_at(index: u8) -> Hasher {
        assert!((index as usize) < N_HASHERS);

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
        let index = self.index as u32;
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
    /// Panics if more than [`N_HASHERS`] hashers are currently in use.
    /// Throws a syscall exception (aborting the frame) if the host call
    /// fails.
    pub fn fork(&self) -> Hasher {
        self.try_fork().unwrap_or_else(|| {
            panic!("Hasher::fork: no more hashers are available");
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
        crate::sys::raw_SHA256Copy(self.index as u32, slot as u32)
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
        crate::sys::raw_SHA256Update(self.index as u32, bytes, true)
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
        let ok =
            crate::sys::raw_SHA256Copy(src.index as u32, self.index as u32)
                .is_ok();
        if ok {
            self.needs_reset = false;
        }
        ok
    }

    /// Consume the hasher and return the SHA256 hash of the prior input.
    ///
    /// Throws a syscall exception (aborting the frame) if a host call fails.
    pub fn finish(mut self) -> SHA256Hash {
        let index = self.index as u32;

        // If the hasher was never updated, the VM has not created a buffer for
        // this slot (and any buffer left over from a previously dropped hasher
        // on the reused slot would be stale). Issue an empty reset update so
        // that SHA256Finalize sees a fresh, empty buffer and returns the hash
        // of the empty input. Mirrors how `update()` invokes the syscall.
        if self.needs_reset {
            self.needs_reset = false;
            crate::sys::raw_SHA256Update(index, &[], true).unwrap_or_else(
                |e| syscall_throw(SyscallNo::kSHA256Update, e),
            );
        }

        let mut output = core::mem::MaybeUninit::uninit();

        crate::sys::uninit_SHA256Finalize(index, &mut output)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Finalize, e));

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
        // We could call `SHA256Finalize` to free the VM slot,
        // but instead we'll leave it be.  If we use the slot again,
        // `SHA256Update` with `reset = true` will reset the slot.
        //
        // A slot that is already free indicates an internal bookkeeping bug,
        // but panicking here would turn an unwind already in progress into a
        // double panic (an immediate process abort), so tolerate it.
        let _already_free = !slot_free(self.index);
    }
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

// A set of flags marking what hashers are available
contract_local! {
    static AVAILABLE: ContractCell<u32> = const { ContractCell::new(0xffff_ffff) };
}

// TODO: rewrite slot_alloc and slot_free to use .update() once
// local_key_cell_update is stabilized (rust#143989). Using get/set for now
// because .update() on LocalKey<Cell<T>> requires nightly, which breaks
// doc builds on stable (e.g. kellnr).
#[cfg(any(doc, feature = "std", target_arch = "riscv32"))]
fn slot_alloc() -> Option<HasherIndex> {
    let mut avail = AVAILABLE.get();

    for i in 0..N_HASHERS {
        if avail & (1 << i) != 0 {
            avail &= !(1 << i);
            AVAILABLE.set(avail);
            return Some(i as _);
        }
    }

    None
}

// Suppress compile errors when building for native without std
#[cfg(not(any(doc, feature = "std", target_arch = "riscv32")))]
fn slot_alloc() -> Option<HasherIndex> {
    None
}

#[cfg(any(doc, feature = "std", target_arch = "riscv32"))]
fn slot_free(i: HasherIndex) -> bool {
    let mut avail = AVAILABLE.get();

    if avail & (1 << i) == 0 {
        avail |= 1 << i;
        AVAILABLE.set(avail);
        true
    } else {
        false
    }
}

#[cfg(not(any(doc, feature = "std", target_arch = "riscv32")))]
fn slot_free(_i: HasherIndex) -> bool {
    false
}

// Regression coverage for the drop-during-unwind double panic.
//
// `Hasher::drop` used to panic when `slot_free` reported the slot as
// already free. Drop runs almost exclusively during unwinds (a throw or
// panic with a live hasher on the stack), and a panic from a destructor
// during an unwind is a double panic: the process aborts immediately,
// killing the whole test binary instead of failing one test.
//
// The already-free state is not reachable through the public API on a
// single thread (slot indices are unforgeable and only `drop` frees
// them), so reconstruct the bookkeeping desync directly with the
// module's own `slot_free`, the same way `test_slots` drives the
// allocator. With the old panicking drop this test aborted the entire
// test process; with the tolerant drop the original panic is the only
// panic and unwinds normally.
#[test]
fn test_drop_of_desynced_slot_during_unwind_is_not_a_double_panic() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let hasher = Hasher::try_new().expect("a hasher slot must be available");

    // Desync the bookkeeping: mark the live hasher's slot free behind its
    // back — the state whose drop used to panic.
    assert!(slot_free(hasher.index), "slot must have been allocated");

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
    use std::vec;

    let mut slots = vec![];

    for _ in 0..N_HASHERS {
        let slot = slot_alloc().expect("No slots available");
        // eprintln!("ALLOC {}", slot);
        slots.push(slot);
    }

    for slot in slots.into_iter() {
        if !slot_free(slot) {
            panic!("Slot {} already freed", slot);
        }
        // eprintln!("FREE {}", slot);
    }
}
