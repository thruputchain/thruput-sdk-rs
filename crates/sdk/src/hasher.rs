use crate::SHA256Hash;
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
    pub fn update<I: AsRef<[u8]>>(&mut self, input: &I) {
        let index = self.index as u32;
        let reset = self.needs_reset;

        self.needs_reset = false;

        crate::sys::raw_SHA256Update(index, input.as_ref(), reset)
            .unwrap_or_else(|e| {
                panic!("SHA256Update: syscall failed: {:?}", e)
            });
    }

    /// Consume the hasher and return the SHA256 hash of the prior input.
    pub fn finish(self) -> SHA256Hash {
        let index = self.index as u32;
        let mut output = core::mem::MaybeUninit::uninit();

        crate::sys::uninit_SHA256Finalize(index, &mut output).unwrap_or_else(
            |e| panic!("SHA256Finalize: syscall failed: {:?}", e),
        );

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
        if !slot_free(self.index) {
            panic!("Hasher::drop: internal error: already freed");
        }
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
#[allow(unused)]
use bitset_core::*;

// A set of flags marking what hashers are available
contract_local! {
    static AVAILABLE: ContractCell<u32> = const { ContractCell::new(0xffff_ffff) };
}

// TODO: rewrite slot_alloc and slot_free to use .update() once
// local_key_cell_update is stabilized (rust#143989). Using get/set for now
// because .update() on LocalKey<Cell<T>> requires nightly, which breaks
// doc builds on stable (e.g. kellnr).
#[cfg(any(
    doc,
    feature = "std",
    target_family = "wasm",
    target_arch = "riscv32"
))]
fn slot_alloc() -> Option<HasherIndex> {
    let mut avail = AVAILABLE.get();

    for i in 0..N_HASHERS {
        if avail.bit_test(i) {
            avail.bit_reset(i);
            AVAILABLE.set(avail);
            return Some(i as _);
        }
    }

    None
}

// Suppress compile errors when building for native without std
#[cfg(not(any(
    doc,
    feature = "std",
    target_family = "wasm",
    target_arch = "riscv32"
)))]
fn slot_alloc() -> Option<HasherIndex> {
    None
}

#[cfg(any(
    doc,
    feature = "std",
    target_family = "wasm",
    target_arch = "riscv32"
))]
fn slot_free(i: HasherIndex) -> bool {
    let mut avail = AVAILABLE.get();

    if !avail.bit_test(i as _) {
        avail.bit_set(i as _);
        AVAILABLE.set(avail);
        true
    } else {
        false
    }
}

#[cfg(not(any(
    doc,
    feature = "std",
    target_family = "wasm",
    target_arch = "riscv32"
)))]
fn slot_free(_i: HasherIndex) -> bool {
    false
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
