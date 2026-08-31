//! The benchmarked operations, one function each.
//!
//! Every op has the same `fn(B, B) -> B` shape, so [`crate::kernel::drive`] is
//! a single loop shared by all of them and the `op-none` build's gas subtracts
//! cleanly. Where an operation needs an operand the type does not supply
//! directly -- a shift distance, a single-limb divisor, a `u128` -- it is
//! *derived* from `a`/`b` by the same expression for every backend, so the
//! derivation cancels in the diff.
//!
//! These are generic, so nothing is emitted for an op the selected kernel does
//! not call: only the one the build's `op-*` feature names gets instantiated.

use crate::backend::Backend;
use core::hint::black_box;

/// Calibration: the loop and the operand round trips, with no arithmetic.
#[inline]
pub fn none<B: Backend>(a: B, _b: B) -> B {
    a
}

#[inline]
pub fn add<B: Backend>(a: B, b: B) -> B {
    a.wrapping_add(b)
}

#[inline]
pub fn sub<B: Backend>(a: B, b: B) -> B {
    a.wrapping_sub(b)
}

#[inline]
pub fn mul<B: Backend>(a: B, b: B) -> B {
    a.wrapping_mul(b)
}

/// Multiply by a single-limb scalar.
#[inline]
pub fn mul_small<B: Backend>(a: B, b: B) -> B {
    a.wrapping_mul_small(b.low_u32())
}

/// The exact 256-bit product of two 128-bit values.
#[inline]
pub fn widening_mul<B: Backend>(a: B, b: B) -> B {
    B::widening_mul_u128(a.low_u128(), b.low_u128())
}

/// General 256-by-256 divide.
///
/// The divisor is `b | 1`: nonzero by construction (both implementations panic
/// on a zero divisor) and still full-width, so the multi-limb path runs. One
/// extra bitwise-or, in both backends.
#[inline]
pub fn div<B: Backend>(a: B, b: B) -> B {
    a.div(b.bitor(B::one()))
}

/// Divide by a single-limb divisor, the common case.
#[inline]
pub fn div_small<B: Backend>(a: B, b: B) -> B {
    a.div_small(b.low_u32() | 1)
}

#[inline]
pub fn rem<B: Backend>(a: B, b: B) -> B {
    a.rem(b.bitor(B::one()))
}

/// `a * b / d` through a 256-bit intermediate.
///
/// `d = b | 1` keeps the quotient inside a `u128` for every input: with `b`
/// odd, `d == b` and the quotient is exactly `a`; with `b` even, `d > b` and it
/// is smaller. `sdk::mul_div_u128` panics when the quotient overflows, so this
/// is a precondition, not a nicety.
#[inline]
pub fn mul_div_u128<B: Backend>(a: B, b: B) -> B {
    let b = b.low_u128();
    B::from_u128(B::mul_div_u128(a.low_u128(), b, b | 1))
}

/// `a * b / d` through an exact 512-bit intermediate.
///
/// `d = b | 1` keeps the quotient inside 256 bits for every input, on the same
/// argument [`mul_div_u128`] uses one width down: with `b` odd, `d == b` and
/// the quotient is exactly `a`; with `b` even, `d > b` and it is smaller. Both
/// backends panic when the quotient overflows, so this is a precondition, not
/// a nicety.
#[inline]
pub fn mul_div<B: Backend>(a: B, b: B) -> B {
    B::mul_div(a, b, b.bitor(B::one()))
}

/// Left shift.
///
/// The distance is reduced modulo the width: `sdk::U256::wrapping_shl` masks
/// the distance the way `u128::wrapping_shl` does, while `uint`'s `Shl` shifts
/// bits off the top. Reducing first is where the two agree.
#[inline]
pub fn shl<B: Backend>(a: B, b: B) -> B {
    a.shl(b.low_u32() % B::BITS)
}

/// Right shift. Same reduction as [`shl`].
#[inline]
pub fn shr<B: Backend>(a: B, b: B) -> B {
    a.shr(b.low_u32() % B::BITS)
}

/// Four bitwise ops in one kernel -- individually they are too cheap to
/// separate from the loop.
#[inline]
pub fn bitwise<B: Backend>(a: B, b: B) -> B {
    a.bitand(b).bitor(a.bitxor(b)).not()
}

#[inline]
pub fn cmp<B: Backend>(a: B, b: B) -> B {
    B::from_u128(u128::from(a.lt(b)))
}

#[inline]
pub fn sqrt<B: Backend>(a: B, _b: B) -> B {
    a.integer_sqrt()
}

/// Big-endian byte round trip: the wire and storage conversion.
///
/// The barrier between the two halves is load-bearing. `from_be_bytes` after
/// `to_be_bytes` is the identity, and `kernel::drive`'s `black_box` sits outside
/// the call, so without one here LLVM folds the pair away entirely -- measured,
/// this op reported a marginal `.text` of exactly zero in both backends before
/// the barrier was added.
#[inline]
pub fn bytes<B: Backend>(a: B, _b: B) -> B {
    B::from_be_bytes(black_box(a.to_be_bytes()))
}

#[inline]
pub fn bit_len<B: Backend>(a: B, _b: B) -> B {
    B::from_u128(u128::from(a.bit_len()))
}
