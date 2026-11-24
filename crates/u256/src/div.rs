//! Division and remainder. Every division is the runtime's `U256MulDivRem`
//! with a multiplier of one, or `U128MulDivRem` at 128 bits.

use crate::{Rounding, U256};

impl U256 {
    /// Divides by a 32-bit value, returning the quotient and remainder.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub fn div_rem_small(self, rhs: u32) -> (Self, u32) {
        let (q, r) = self.div_rem(Self::from_u32(rhs));
        (q, r.limbs[0])
    }

    /// Divides by another `U256`, returning the quotient and remainder.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero, matching the primitive integers (which panic on a
    /// zero divisor regardless of `overflow-checks`).
    #[inline]
    pub fn div_rem(self, rhs: Self) -> (Self, Self) {
        if rhs.is_zero() {
            panic!("attempt to divide by zero");
        }
        crate::host::div_rem(&self, &rhs)
    }

    /// Divides by another `U256`, returning `None` if `rhs` is zero.
    #[inline]
    pub fn checked_div_rem(self, rhs: Self) -> Option<(Self, Self)> {
        if rhs.is_zero() {
            return None;
        }
        Some(crate::host::div_rem(&self, &rhs))
    }

    /// Division, returning `None` if `rhs` is zero.
    #[inline]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        Some(crate::host::div(&self, &rhs))
    }

    /// Remainder, returning `None` if `rhs` is zero.
    #[inline]
    pub fn checked_rem(self, rhs: Self) -> Option<Self> {
        if rhs.is_zero() {
            return None;
        }
        Some(crate::host::rem(&self, &rhs))
    }
}

/// Computes `a * b / d` exactly, without the intermediate product needing to
/// fit in 128 bits.
///
/// This is the case that sends contract authors looking for a 256-bit type in
/// the first place: two 128-bit amounts multiplied and scaled back down. The
/// `U256` never escapes into caller code.
///
/// # Panics
///
/// If `d` is zero, or if the quotient does not fit in a `u128`. The overflow
/// panic is unconditional rather than `overflow-checks`-gated: this is an
/// explicit API rather than an operator, and silently wrapping the result of a
/// deliberate exact-arithmetic call has no primitive behaviour to match.
#[inline]
pub fn mul_div_u128(a: u128, b: u128, d: u128) -> u128 {
    match checked_mul_div_u128(a, b, d) {
        Some(v) => v,
        None if d == 0 => panic!("attempt to divide by zero"),
        None => panic!("mul_div_u128 quotient exceeds u128"),
    }
}

/// Computes `a * b / d` exactly, returning `None` if `d` is zero or the
/// quotient does not fit in a `u128`. The runtime's `U128MulDivRem`.
#[inline]
pub fn checked_mul_div_u128(a: u128, b: u128, d: u128) -> Option<u128> {
    if d == 0 {
        return None;
    }
    crate::host::mul_div_u128(&a, &b, &d)
}

/// Computes `a * b / d` exactly on signed amounts, truncating toward zero,
/// without the intermediate product needing to fit in 128 bits.
///
/// The signed counterpart of [`mul_div_u128`], for the `i128` amounts the
/// token interfaces use: a pro-rata share needs no casts in or out. The sign
/// rule is the one `/` uses, so wherever `a * b` fits in an `i128` the result
/// is `a * b / d`. See [`mul_div_rounding_i128`] for the other directions.
///
/// # Panics
///
/// If `d` is zero, or if the quotient does not fit in an `i128`. Both panics
/// are unconditional rather than `overflow-checks`-gated, as for
/// [`mul_div_u128`].
#[inline]
pub fn mul_div_i128(a: i128, b: i128, d: i128) -> i128 {
    match checked_mul_div_i128(a, b, d) {
        Some(v) => v,
        None if d == 0 => panic!("attempt to divide by zero"),
        None => panic!("mul_div_i128 quotient out of range for i128"),
    }
}

/// Computes `a * b / d` exactly, truncating toward zero, returning `None` if
/// `d` is zero or the quotient does not fit in an `i128`.
#[inline]
pub fn checked_mul_div_i128(a: i128, b: i128, d: i128) -> Option<i128> {
    mul_div_i128_parts(a, b, d).map(|(q, _, _)| q)
}

/// Computes `a * b / d` exactly, following `rounding`.
///
/// [`Rounding::Trunc`] is [`mul_div_i128`]. For a non-negative result,
/// [`Rounding::Floor`] is the same and [`Rounding::Ceil`] rounds up; the
/// directions differ only below zero.
///
/// # Panics
///
/// As [`mul_div_i128`], on the rounded result.
#[inline]
pub fn mul_div_rounding_i128(
    a: i128,
    b: i128,
    d: i128,
    rounding: Rounding,
) -> i128 {
    match checked_mul_div_rounding_i128(a, b, d, rounding) {
        Some(v) => v,
        None if d == 0 => panic!("attempt to divide by zero"),
        None => panic!("mul_div_i128 quotient out of range for i128"),
    }
}

/// Computes `a * b / d` exactly, following `rounding`, returning `None` if
/// `d` is zero or the rounded result does not fit in an `i128`.
#[inline]
pub fn checked_mul_div_rounding_i128(
    a: i128,
    b: i128,
    d: i128,
    rounding: Rounding,
) -> Option<i128> {
    let (q, inexact, negative) = mul_div_i128_parts(a, b, d)?;
    if !inexact || !rounding.rounds_away_from_zero(negative) {
        return Some(q);
    }
    // Rounding away from zero can leave the range the truncated quotient
    // just fit into.
    if negative {
        q.checked_sub(1)
    } else {
        q.checked_add(1)
    }
}

/// The truncated quotient of `a * b / d`, whether it was inexact, and the
/// sign it carries, or `None` when `d` is zero or the quotient is out of
/// range. The magnitudes go through the runtime's `U128MulDivRem`, in one
/// call.
///
/// The sign is returned because a zero quotient loses it, and the rounding
/// direction of an inexact one still depends on it.
#[inline]
fn mul_div_i128_parts(
    a: i128,
    b: i128,
    d: i128,
) -> Option<(i128, bool, bool)> {
    if d == 0 {
        return None;
    }
    let negative = ((a < 0) != (b < 0)) != (d < 0);
    let (mag, inexact) = crate::host::mul_div_u128_inexact(
        &a.unsigned_abs(),
        &b.unsigned_abs(),
        &d.unsigned_abs(),
    )?;
    // The negative range reaches one further: `0 - 2^127` is `i128::MIN`.
    let q = if negative {
        0_i128.checked_sub_unsigned(mag)?
    } else {
        i128::try_from(mag).ok()?
    };
    Some((q, inexact, negative))
}
