use crate::SHA256Hash;
use crate::exn::syscall_throw;
use crate::sys::{SysResult, SyscallNo};
use core::fmt::*;

/// The maximum number of hashers that can be in existence at once: the VM's
/// live-stream cap (`kMaxSHA256Buffers`). Finishing or dropping a hasher
/// frees its stream.
pub const N_HASHERS: usize = 8;

type HasherIndex = u32;

/// An incremental hasher, implemented by the VM.
///
/// A value of this type is meaningful only in the current contract invocation,
/// so it can't be stored.
pub struct Hasher {
    index: HasherIndex,
}

impl Hasher {
    /// Get a hasher.
    ///
    /// Throws a syscall exception if the VM refuses the stream — its
    /// live-stream cap ([`N_HASHERS`]) is reached.
    pub fn new() -> Hasher {
        Hasher::try_new()
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Update, e))
    }

    /// Get a hasher if the VM has a free stream, or the refusing syscall's
    /// error.
    pub fn try_new() -> SysResult<Hasher> {
        let index = next_index();
        // Create the VM-side stream now (an empty reset update): every live
        // hasher owns a stream, so cap exhaustion surfaces here as the real
        // syscall error rather than on some later use.
        crate::sys::raw_SHA256Update(index, &[], true)?;
        Ok(Hasher { index })
    }

    /// Append data to the hasher input.
    ///
    /// Throws a syscall exception (aborting the frame) if the host call
    /// fails.
    pub fn update<I: AsRef<[u8]>>(&mut self, input: &I) {
        crate::sys::raw_SHA256Update(self.index, input.as_ref(), false)
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
    /// Throws a syscall exception if the VM refuses the copy — its
    /// live-stream cap ([`N_HASHERS`]) is reached.
    pub fn fork(&self) -> Hasher {
        self.try_fork()
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Copy, e))
    }

    /// Fork the hasher if the VM has a free stream (see
    /// [`fork`](Self::fork)), or the refusing syscall's error.
    pub fn try_fork(&self) -> SysResult<Hasher> {
        let index = next_index();
        crate::sys::raw_SHA256Copy(self.index, index)?;
        Ok(Hasher { index })
    }

    /// Crate-internal: replace this hasher's stream with a fresh one holding
    /// exactly `bytes`, reporting failure instead of throwing. Used by the
    /// storage layer's streamed key derivation to rebuild its cached prefix
    /// midstate in place — including when the cached stream is stale (VM
    /// streams are per-invocation; the contract-local cache is not, on
    /// native hosts).
    pub(crate) fn try_reabsorb(&mut self, bytes: &[u8]) -> SysResult {
        crate::sys::raw_SHA256Update(self.index, bytes, true)
    }

    /// Consume the hasher and return the SHA256 hash of the prior input,
    /// freeing its VM-side stream.
    ///
    /// Throws a syscall exception (aborting the frame) if the host call
    /// fails.
    pub fn finish(self) -> SHA256Hash {
        let index = self.index;
        // The finalize below frees the stream itself; skip Drop's discard.
        core::mem::forget(self);

        let mut output = core::mem::MaybeUninit::uninit();

        crate::sys::uninit_SHA256Finalize(index, &mut output)
            .unwrap_or_else(|e| syscall_throw(SyscallNo::kSHA256Finalize, e));

        // SAFETY: The syscall succeeded, so the output has been initialized
        unsafe { output.assume_init() }
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // Free the VM-side stream by finalizing it and discarding the hash:
        // indices are never reused, so nothing else reclaims it. The result
        // is deliberately ignored — the stream may legitimately be gone (a
        // stale cached handle from a previous invocation on native hosts).
        // Skipped during native unwinds: the frame is being torn down and
        // its streams die with it, and a syscall answered with an
        // unrecoverable host state there would turn the unwind into a
        // double panic (an immediate process abort). rv32 guests do not
        // unwind — a contract panic aborts the frame — so there drops never
        // run mid-panic.
        if !unwinding() {
            let mut discard = core::mem::MaybeUninit::uninit();
            let _ =
                crate::sys::uninit_SHA256Finalize(self.index, &mut discard);
        }
    }
}

#[cfg(all(not(any(doc, target_arch = "riscv32")), feature = "std"))]
fn unwinding() -> bool {
    std::thread::panicking()
}

#[cfg(any(doc, target_arch = "riscv32", not(feature = "std")))]
fn unwinding() -> bool {
    false
}

impl Default for Hasher {
    fn default() -> Self {
        Hasher::new()
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

// Stream indices are just names: the VM keys streams by arbitrary u32 and
// caps only how many are live at once, so allocation is a bare counter and
// indices are never reused or reclaimed (u32 does not run out within an
// invocation). Capacity is the VM's to enforce — creation syscalls fail at
// the cap.
contract_local! {
    static NEXT_INDEX: ContractCell<u32> = const { ContractCell::new(0) };
}

#[cfg(any(doc, feature = "std", target_arch = "riscv32"))]
fn next_index() -> HasherIndex {
    let index = NEXT_INDEX.get();
    NEXT_INDEX.set(index + 1);
    index
}

// Suppress compile errors when building for native without std
#[cfg(not(any(doc, feature = "std", target_arch = "riscv32")))]
fn next_index() -> HasherIndex {
    0
}

#[test]
fn test_indices_are_never_reused() {
    let mut last = next_index();
    for _ in 0..2 * N_HASHERS {
        let index = next_index();
        assert!(index > last, "indices must increase monotonically");
        last = index;
    }
}
