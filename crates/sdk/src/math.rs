//! Arithmetic host operations.
//!
//! Wide-integer routines the runtime carries out on its own machine, for a
//! contract that would otherwise run them a 32-bit limb at a time. Each one
//! returns exactly what the software routine in [`u256`](crate::u256) returns.

use crate::U256;
use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};

/// Experimental API: the integer square root of a 256-bit value, computed by
/// the runtime.
///
/// Returns the largest `r` such that `r * r <= value`, the value
/// [`U256::integer_sqrt`] returns. It always fits a `u128`.
///
/// The price is flat, whatever the value, and needs a runtime that
/// implements syscall API 1.2. `U256::integer_sqrt` runs anywhere and is
/// usable in a `const`.
///
/// Throws a syscall exception (aborting the frame) if the host call fails.
pub fn u256_sqrt(value: U256) -> u128 {
    let mut root = core::mem::MaybeUninit::<u128>::uninit();
    sys::uninit_U256Sqrt(le_bytes(&value), &mut root)
        .or_syscall_throw(SyscallNo::kU256Sqrt);
    // SAFETY: a successful syscall initializes the root.
    unsafe { root.assume_init() }
}

/// A [`U256`] as a host operation reads one: 32 bytes, least significant
/// first.
///
/// The limbs are stored least significant first, so on a little-endian
/// target, which rv32 and every host the native harness runs on are, the
/// value's memory already is those bytes and nothing is copied.
fn le_bytes(value: &U256) -> &[u8; 32] {
    const { assert!(cfg!(target_endian = "little")) };
    // SAFETY: `[u32; 8]` and `[u8; 32]` have the same size, a `u8` may sit at
    // any address and hold any value, and the result borrows `value`.
    unsafe { &*core::ptr::from_ref(value.as_limbs()).cast::<[u8; 32]>() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bytes handed to the host are the value's, least significant
    /// first, and are the value's own memory.
    #[test]
    fn le_bytes_are_the_value_in_place() {
        let value = U256::from_limbs([
            0x0403_0201,
            0x0807_0605,
            0x0c0b_0a09,
            0x100f_0e0d,
            0x1413_1211,
            0x1817_1615,
            0x1c1b_1a19,
            0x201f_1e1d,
        ]);
        let bytes = le_bytes(&value);
        let counting: [u8; 32] = core::array::from_fn(|i| i as u8 + 1);
        assert_eq!(*bytes, counting);
        assert!(core::ptr::eq(
            bytes.as_ptr(),
            value.as_limbs().as_ptr().cast::<u8>()
        ));

        let mut reversed = *bytes;
        reversed.reverse();
        assert_eq!(reversed, value.to_be_bytes());
    }
}
