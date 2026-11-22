//! The runtime's arithmetic host operations, which a contract calls in place
//! of this crate's software routines. Built for `riscv32` only.

use crate::{LIMBS, U256};

/// `value` as the runtime reads a 256-bit integer: 32 bytes, least significant
/// first. That is the limbs in the order they are stored, each least
/// significant byte first.
///
/// Borrowing the limbs as those bytes, which needs `unsafe`, was measured
/// against this copy in the `sqrt` bench kernel and compiled to the same code:
/// the value arrives by value, and its one trip to memory is this.
#[inline]
pub(crate) fn le_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (chunk, limb) in
        bytes.as_chunks_mut::<4>().0.iter_mut().zip(value.limbs)
    {
        *chunk = limb.to_le_bytes();
    }
    bytes
}

/// The value of 32 bytes the runtime wrote, least significant first.
#[inline]
pub(crate) fn from_le_bytes(bytes: [u8; 32]) -> U256 {
    let mut limbs = [0u32; LIMBS];
    for (limb, chunk) in limbs.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *limb = u32::from_le_bytes(*chunk);
    }
    U256::from_limbs(limbs)
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
