use crate::Cell;
use crate::SHA256Hash;
use bitset_core::*;
use core::fmt::*;

/// The maximum number of hashers that can be in existence at once
pub const N_HASHERS: usize = 8;

type HasherIndex = u8;

/// An incremental hasher
pub struct Hasher {
    index: HasherIndex,
    needs_reset: bool,
}

impl Hasher {
    /// Get a hasher.  Panics if more than `N_HASHERS` hashers are currently in use.
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
    pub fn update(&mut self, input: &[u8]) {
        let index = self.index as u32;
        let reset = self.needs_reset;

        self.needs_reset = false;

        crate::sys::raw_SHA256Update(index, input, reset)
            .unwrap_or_else(|e| panic!("SHA256Update: syscall failed: {}", e));
    }

    /// Consume the hasher and return the SHA256 hash of the prior input.
    pub fn finish(self) -> SHA256Hash {
        let index = self.index as u32;
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

// A set of flags marking what hashers are available
static AVAILABLE: Cell<u32> = Cell::new(0xffff_ffff);

fn slot_alloc() -> Option<HasherIndex> {
    let mut slot = None;

    AVAILABLE.update(|mut avail| {
        for i in 0..N_HASHERS {
            if avail.bit_test(i) {
                // This slot is available
                slot = Some(i as _);

                // Mark it not available
                avail.bit_reset(i);

                break;
            }
        }

        // Return updated state
        avail
    });

    slot
}

fn slot_free(i: HasherIndex) -> bool {
    let mut success = false;

    AVAILABLE.update(|mut avail| {
        if !avail.bit_test(i as _) {
            // Mark it available
            avail.bit_set(i as _);

            success = true;
        }

        // Return the modified bitmap
        avail
    });

    success
}

#[cfg(test)]
mod hash_test {
    use super::*;

    /*
    use std::sync::Mutex;
    static BOX: Mutex<u32> = Mutex::new(0);

    #[test]
    fn test_mutex() {
        let val = {
            let mut v = BOX.lock().unwrap();
            *v += 1;
            *v
        };
        std::eprintln!("BOX: {}", val);
    }

    #[test]
    fn test_mutex2() {
        let val = {
            let mut v = BOX.lock().unwrap();
            *v += 1;
            *v
        };
        std::eprintln!("BOX: {}", val);
    }
    */

    fn _test_slots() {
        use std::eprintln;
        use std::vec;

        eprintln!("AVAILABLE: {:?}", BitFmt2(AVAILABLE.get()));

        let mut slots = vec![];

        for _ in 0..N_HASHERS/2 {
            let slot = slot_alloc().expect("No slots available");
            eprintln!("ALLOC {}", slot);
            slots.push(slot);
        }

        eprintln!("ALLOCATED: {:?}", BitFmt2(AVAILABLE.get()));

        for slot in slots.into_iter() {
            if !slot_free(slot) {
                panic!("Slot {} already freed", slot);
            }
            eprintln!("FREE {}", slot);
        }

        eprintln!("FREED: {:?}", BitFmt2(AVAILABLE.get()));
    }

    #[test]
    fn test_slots_1() {
        _test_slots();
    }

    #[test]
    fn test_slots_2() {
        _test_slots();
    }

    struct BitFmt2<T: ?Sized + BitSet>(T);

    impl<T: ?Sized + BitSet> Debug for BitFmt2<T> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            bitstring(&self.0, f)
        }
    }

    fn bitstring<T: ?Sized + BitSet>(this: &T, f: &mut Formatter) -> Result {
            const ALPHABET: [u8; 2] = [b'0', b'1'];
            let mut buf = [0u8; 9];
            let mut first = true;
            buf[0] = b'_';
            let mut i = 0;
            while i < this.bit_len() {
                    buf[1] = ALPHABET[this.bit_test(i + 0) as usize];
                    buf[2] = ALPHABET[this.bit_test(i + 1) as usize];
                    buf[3] = ALPHABET[this.bit_test(i + 2) as usize];
                    buf[4] = ALPHABET[this.bit_test(i + 3) as usize];
                    buf[5] = ALPHABET[this.bit_test(i + 4) as usize];
                    buf[6] = ALPHABET[this.bit_test(i + 5) as usize];
                    buf[7] = ALPHABET[this.bit_test(i + 6) as usize];
                    buf[8] = ALPHABET[this.bit_test(i + 7) as usize];
                    let s = unsafe { &*((&buf[first as usize..]) as *const _ as *const str) };
                    f.write_str(s)?;
                    i += 8;
                    first = false;
            }
            Ok(())
    }
}
