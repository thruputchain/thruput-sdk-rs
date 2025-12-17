use crate::Cell;
use crate::SHA256Hash;
use bitset_core::*;

/// An incremental hasher
pub struct Hasher(u8);

/// The maximum number of hashers that can be in existence at once
pub const N_HASHERS: usize = 32;

static AVAILABLE: Cell<u32> = Cell::new(0xffffffff);

impl Hasher {
    /// Get a hasher.  Panics if more than `N_HASHERS` hashers are currently in use.
    pub fn new() -> Hasher {
        Hasher::try_new().unwrap_or_else(|| {
            panic!("Hasher::new: no hashers are available");
        })
    }

    /// Get a hasher if one is available.
    pub fn try_new() -> Option<Hasher> {
        let mut avail = AVAILABLE.get();

        for i in 0..N_HASHERS {
            if avail.bit_test(i) {
                // Mark the slot not available
                avail.bit_reset(i);

                // Store the modified bitmap
                AVAILABLE.set(avail);

                return Some(Hasher(i as u8));
            }
        }

        // AVAILABLE was not modified
        return None;
    }

    pub fn update(&self, input: &[u8]) {
        let index = self.0 as u32;
        let reset = false;

        crate::sys::raw_SHA256Update(index, input, reset)
            .unwrap_or_else(|e| panic!("SHA256Update: syscall failed: {}", e));
    }

    pub fn finish(&self) -> SHA256Hash {
        let index = self.0 as u32;
        let mut output = core::mem::MaybeUninit::uninit();

        crate::sys::uninit_SHA256Finalize(index, &mut output).unwrap_or_else(
            |e| panic!("SHA256Finalize: syscall failed: {}", e),
        );

        // SAFETY: The syscall succeeded, so the output has been initialized
        unsafe { output.assume_init() }
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // Get the index of this hasher
        let i = self.0 as usize;

        let mut avail = AVAILABLE.get();

        assert!(
            !avail.bit_test(i),
            "Hasher::drop: internal error: already freed"
        );

        // Mark it available
        avail.bit_set(i);

        // Store the modified bitmap
        AVAILABLE.set(avail);
    }
}
