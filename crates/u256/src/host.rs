//! The runtime's arithmetic host operations, through their generated
//! bindings. Every division on these types is one of these calls: an `ecall`
//! in a contract, the native executor in a native test.

use crate::{LIMBS, U256};

// The runtime reads a 256-bit integer as 32 bytes, least significant first.
// The limbs are stored least significant first, so on a little-endian target
// their bytes already are those 32.
const _: () = assert!(cfg!(target_endian = "little"));

/// `value`'s bytes, as the runtime reads them.
#[inline]
pub(crate) fn le_bytes(value: U256) -> [u8; 32] {
    // SAFETY: the two types have the same size, and every bit pattern is
    // valid for both.
    unsafe { core::mem::transmute::<[u32; LIMBS], [u8; 32]>(value.limbs) }
}

/// The value of 32 bytes the runtime wrote.
#[inline]
pub(crate) fn from_le_bytes(bytes: [u8; 32]) -> U256 {
    // SAFETY: as for `le_bytes`.
    U256::from_limbs(unsafe {
        core::mem::transmute::<[u8; 32], [u32; LIMBS]>(bytes)
    })
}

/// `x * y` divided by `d`, by the `U256MulDivRem` host operation: the
/// quotient's low and high 256 bits, then the remainder.
///
/// # Panics
///
/// If the host operation fails, which it does for a zero `d`. Callers that
/// answer `None` for a zero divisor check it first.
pub(crate) fn mul_div_rem(x: U256, y: U256, d: U256) -> (U256, U256, U256) {
    match thruput_xdr::syscalls::U256MulDivRem(
        &le_bytes(x),
        &le_bytes(y),
        &le_bytes(d),
    ) {
        Ok((lo, hi, rem)) => {
            (from_le_bytes(lo), from_le_bytes(hi), from_le_bytes(rem))
        }
        Err(_) => panic!("U256MulDivRem failed"),
    }
}

/// `a * b` divided by `d`, by the `U128MulDivRem` host operation: the
/// quotient's low and high 128 bits, then the remainder.
///
/// # Panics
///
/// As [`mul_div_rem`].
pub(crate) fn mul_div_rem_u128(
    a: u128,
    b: u128,
    d: u128,
) -> (u128, u128, u128) {
    match thruput_xdr::syscalls::U128MulDivRem(&a, &b, &d) {
        Ok(parts) => parts,
        Err(_) => panic!("U128MulDivRem failed"),
    }
}
