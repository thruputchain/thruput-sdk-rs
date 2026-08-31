//! Signed math utilities: OpenZeppelin's `SignedMath.sol`, in Rust, plus the
//! signed half of the `mulDiv` family.
//!
//! `SignedMath.sol` is five functions. Four have direct counterparts here
//! ([`min`](I256::min), [`max`](I256::max), [`average`](I256::average),
//! and `abs` -- which is [`unsigned_abs`](I256::unsigned_abs), already total
//! and already returning a [`U256`] exactly as Solidity's does). The fifth,
//! `ternary`, is a branchless-select trick for the EVM's gas model; in Rust it
//! is `if`.
//!
//! Solidity has no signed `mulDiv`, so [`mul_div`](I256::mul_div) here is the
//! obvious composition rather than a port: magnitudes go through
//! [`U256::mul_div`], and the sign comes back afterwards. It truncates toward
//! zero, the same rule `/` uses, and the rounding variant takes the same
//! [`Rounding`] the unsigned side does -- where the two signed directions stop
//! coinciding, and [`Rounding::rounds_away_from_zero`] is what decides.

use crate::{I256, Rounding, U256};

impl I256 {
    // -----------------------------------------------------------------------
    // Comparison and averaging
    // -----------------------------------------------------------------------

    /// The smaller of two values. `SignedMath.min`.
    ///
    /// Shadows [`Ord::min`] in method position, which resolves to the same
    /// answer; this one is a `const fn`.
    #[inline]
    pub const fn min(self, other: Self) -> Self {
        if self.const_lt(other) { self } else { other }
    }

    /// The larger of two values. `SignedMath.max`.
    ///
    /// Shadows [`Ord::max`] in method position, which resolves to the same
    /// answer; this one is a `const fn`.
    #[inline]
    pub const fn max(self, other: Self) -> Self {
        if self.const_gt(other) { self } else { other }
    }

    /// Clamps to the inclusive range `min..=max`.
    ///
    /// # Panics
    ///
    /// If `min > max`, matching [`Ord::clamp`].
    #[inline]
    pub const fn clamp(self, min: Self, max: Self) -> Self {
        if min.const_gt(max) {
            panic!("min must be less than or equal to max");
        }
        self.max(min).min(max)
    }

    /// The average of two values, **rounded toward zero**, without an
    /// intermediate that overflows. `SignedMath.average`.
    ///
    /// `(a & b) + ((a ^ b) >> 1)` is the floor average -- the shift is
    /// arithmetic, so it rounds toward negative infinity -- and the
    /// correction below pulls a negative, inexact result back up by one.
    /// Note that this differs from [`U256::average`] only in that
    /// correction, because for a value that cannot be negative the two
    /// directions coincide.
    #[inline]
    pub const fn average(self, other: Self) -> Self {
        let xor = self.const_bitxor(other);
        let floor = self.const_bitand(other).wrapping_add(xor.wrapping_shr(1));
        // The sum was odd (so the halving lost a half) and the result is
        // below zero (so "toward zero" means up).
        if floor.is_negative() && xor.bit(0) {
            floor.wrapping_add(Self::ONE)
        } else {
            floor
        }
    }

    // -----------------------------------------------------------------------
    // Full-precision multiply-divide
    // -----------------------------------------------------------------------

    /// `x * y / d` computed on the exact 512-bit product, truncating toward
    /// zero. Returns `None` if `d` is zero or the quotient is out of range.
    ///
    /// The signed counterpart of [`U256::mul_div`]; see the module docs for
    /// why it is a composition rather than a port.
    #[inline]
    pub const fn checked_mul_div(x: Self, y: Self, d: Self) -> Option<Self> {
        match Self::mul_div_parts(x, y, d) {
            Some((q, _, _)) => Some(q),
            None => None,
        }
    }

    /// `x * y / d` computed on the exact 512-bit product, truncating toward
    /// zero.
    ///
    /// # Panics
    ///
    /// If `d` is zero, or if the quotient is out of range. Both panics are
    /// unconditional, as on the unsigned side.
    #[inline]
    pub const fn mul_div(x: Self, y: Self, d: Self) -> Self {
        match Self::checked_mul_div(x, y, d) {
            Some(v) => v,
            None => {
                if d.is_zero() {
                    panic!("attempt to divide by zero");
                }
                panic!("mul_div quotient out of range for I256");
            }
        }
    }

    /// `x * y / d` computed on the exact 512-bit product, following
    /// `rounding`. Returns `None` if `d` is zero or the result is out of
    /// range.
    #[inline]
    pub const fn checked_mul_div_rounding(
        x: Self,
        y: Self,
        d: Self,
        rounding: Rounding,
    ) -> Option<Self> {
        let (q, inexact, negative) = match Self::mul_div_parts(x, y, d) {
            Some(v) => v,
            None => return None,
        };
        if !inexact || !rounding.rounds_away_from_zero(negative) {
            return Some(q);
        }
        if negative {
            q.checked_sub(Self::ONE)
        } else {
            q.checked_add(Self::ONE)
        }
    }

    /// `x * y / d` computed on the exact 512-bit product, following
    /// `rounding`.
    ///
    /// # Panics
    ///
    /// As [`mul_div`](Self::mul_div).
    #[inline]
    pub const fn mul_div_rounding(
        x: Self,
        y: Self,
        d: Self,
        rounding: Rounding,
    ) -> Self {
        match Self::checked_mul_div_rounding(x, y, d, rounding) {
            Some(v) => v,
            None => {
                if d.is_zero() {
                    panic!("attempt to divide by zero");
                }
                panic!("mul_div quotient out of range for I256");
            }
        }
    }

    /// The truncated quotient of `x * y / d`, whether it was exact, and the
    /// sign it carries -- or `None` when `d` is zero or the quotient is out of
    /// range.
    ///
    /// The sign is returned rather than re-derived because the quotient may be
    /// zero, which loses it, and the rounding direction of an inexact
    /// zero-magnitude result still depends on which side of zero it came from.
    #[inline]
    const fn mul_div_parts(
        x: Self,
        y: Self,
        d: Self,
    ) -> Option<(Self, bool, bool)> {
        let negative = (x.is_negative() != y.is_negative()) != d.is_negative();

        let (mag, rem) = match U256::mul_div_parts(
            x.unsigned_abs(),
            y.unsigned_abs(),
            d.unsigned_abs(),
        ) {
            Some(v) => v,
            None => return None,
        };

        // The negative range reaches one further, so the bound is on the
        // magnitude rather than on the value it would become.
        let limit = if negative {
            U256::ONE.wrapping_shl(255)
        } else {
            Self::MAX.cast_unsigned()
        };
        if mag.const_gt(limit) {
            return None;
        }

        let q = if negative { mag.wrapping_neg() } else { mag };
        Some((q.cast_signed(), !rem.is_zero(), negative))
    }
}
