//! Division and remainder. Every division is the runtime's `U256MulDivRem`
//! with a multiplier of one, or `U128MulDivRem` at 128 bits.

use crate::U256;

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
