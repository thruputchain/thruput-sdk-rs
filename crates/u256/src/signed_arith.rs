//! [`I256`] arithmetic: operators, and the `checked_`/`wrapping_`/
//! `overflowing_`/`saturating_` families behind them.
//!
//! Two's complement does most of the work. Addition, subtraction,
//! multiplication and every bitwise operation produce the same 256 bits
//! whether the operands are read as signed or unsigned, so those route
//! straight to [`U256`] and only the *overflow test* is signed. What is
//! genuinely different lives here: sign-aware division, the arithmetic right
//! shift, and negation.
//!
//! Overflow behaves as it does on the unsigned side and as it does for the
//! primitives -- wrap when `overflow-checks` is off, panic when it is on, with
//! every guard folding away in a release contract build. `MIN` is the one
//! value with no negation, which makes `-MIN`, `MIN.abs()` and `MIN / -1`
//! overflow.

use crate::arith::OVERFLOW_CHECKS;
use crate::{I256, U256};
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor,
    BitXorAssign, Div, DivAssign, Mul, MulAssign, Neg, Not, Rem, RemAssign,
    Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

impl I256 {
    // -----------------------------------------------------------------------
    // Negation and absolute value
    // -----------------------------------------------------------------------

    /// Negation, with a flag reporting whether it overflowed.
    ///
    /// Only [`MIN`](Self::MIN) overflows: its magnitude has no positive
    /// counterpart, and it negates to itself.
    #[inline]
    pub const fn overflowing_neg(self) -> (Self, bool) {
        (
            self.cast_unsigned().wrapping_neg().cast_signed(),
            self.const_eq(Self::MIN),
        )
    }

    /// Negation, returning `None` on overflow.
    #[inline]
    pub const fn checked_neg(self) -> Option<Self> {
        match self.overflowing_neg() {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Negation, wrapping on overflow (`MIN` negates to itself).
    #[inline]
    pub const fn wrapping_neg(self) -> Self {
        self.overflowing_neg().0
    }

    /// Negation, saturating at [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_neg(self) -> Self {
        match self.overflowing_neg() {
            (v, false) => v,
            (_, true) => Self::MAX,
        }
    }

    /// Absolute value, with a flag reporting whether it overflowed.
    #[inline]
    pub const fn overflowing_abs(self) -> (Self, bool) {
        if self.is_negative() {
            self.overflowing_neg()
        } else {
            (self, false)
        }
    }

    /// Absolute value, returning `None` on overflow.
    #[inline]
    pub const fn checked_abs(self) -> Option<Self> {
        match self.overflowing_abs() {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Absolute value, wrapping on overflow (`MIN` maps to itself).
    #[inline]
    pub const fn wrapping_abs(self) -> Self {
        self.overflowing_abs().0
    }

    /// Absolute value, saturating at [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_abs(self) -> Self {
        match self.overflowing_abs() {
            (v, false) => v,
            (_, true) => Self::MAX,
        }
    }

    /// Absolute value.
    ///
    /// See [`unsigned_abs`](Self::unsigned_abs) for the total version: `MIN`
    /// has no representable absolute value here.
    ///
    /// # Panics
    ///
    /// On `MIN`, if `overflow-checks` is on -- otherwise it wraps to `MIN`,
    /// like the primitives.
    #[inline]
    pub const fn abs(self) -> Self {
        let (v, overflow) = self.overflowing_abs();
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to negate with overflow");
        }
        v
    }

    // -----------------------------------------------------------------------
    // Addition and subtraction
    // -----------------------------------------------------------------------

    /// Wrapping addition, with a flag reporting whether it overflowed.
    #[inline]
    pub const fn overflowing_add(self, rhs: Self) -> (Self, bool) {
        let v = self
            .cast_unsigned()
            .wrapping_add(rhs.cast_unsigned())
            .cast_signed();
        // Signed overflow is exactly: two operands of the same sign producing
        // a result of the other one.
        let overflow = self.is_negative() == rhs.is_negative()
            && v.is_negative() != self.is_negative();
        (v, overflow)
    }

    /// Wrapping subtraction, with a flag reporting whether it overflowed.
    #[inline]
    pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
        let v = self
            .cast_unsigned()
            .wrapping_sub(rhs.cast_unsigned())
            .cast_signed();
        // Mirror image: operands of opposite signs, result taking the
        // subtrahend's.
        let overflow = self.is_negative() != rhs.is_negative()
            && v.is_negative() != self.is_negative();
        (v, overflow)
    }

    /// Addition, returning `None` on overflow.
    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.overflowing_add(rhs) {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Subtraction, returning `None` on overflow.
    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.overflowing_sub(rhs) {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Addition, wrapping on overflow.
    #[inline]
    pub const fn wrapping_add(self, rhs: Self) -> Self {
        self.overflowing_add(rhs).0
    }

    /// Subtraction, wrapping on overflow.
    #[inline]
    pub const fn wrapping_sub(self, rhs: Self) -> Self {
        self.overflowing_sub(rhs).0
    }

    /// Addition, saturating at [`MIN`](Self::MIN) or [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        match self.overflowing_add(rhs) {
            (v, false) => v,
            // Same-sign operands are what overflowed, so the direction is
            // whichever sign they shared.
            (_, true) => self.saturation_bound(),
        }
    }

    /// Subtraction, saturating at [`MIN`](Self::MIN) or [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        match self.overflowing_sub(rhs) {
            (v, false) => v,
            (_, true) => self.saturation_bound(),
        }
    }

    /// The bound an overflow away from `self` saturates to: an overflow can
    /// only ever leave the range on `self`'s own side of zero.
    #[inline]
    const fn saturation_bound(self) -> Self {
        if self.is_negative() {
            Self::MIN
        } else {
            Self::MAX
        }
    }

    // -----------------------------------------------------------------------
    // Multiplication
    // -----------------------------------------------------------------------

    /// Wrapping multiplication, with a flag reporting whether it overflowed.
    ///
    /// One 256x256 multiply, not two. The overflow test needs the magnitude
    /// product, and the wrapped *signed* bits follow from it without a second
    /// pass: `a * b = (-1)^(sign a ^ sign b) * |a| * |b|`, and that identity
    /// survives reduction mod 2^256, so negating the magnitude product gives
    /// exactly the bits `cast_unsigned().wrapping_mul()` would have.
    ///
    /// Which callers this helps is worth being precise about. It is every form
    /// that *consumes* the overflow flag -- [`checked_mul`](Self::checked_mul),
    /// [`saturating_mul`](Self::saturating_mul), this function, and each step
    /// of [`pow`](Self::pow). The bare `*` operator is not among them: in a
    /// release contract profile `OVERFLOW_CHECKS` folds to false, the flag goes
    /// unread, and LLVM was already dropping the second multiply as dead.
    ///
    /// Measured on rv32 by `scripts/bigint-bench.sh -o "mul signed_mul"`: a
    /// saturating signed multiply costs 1095.02 gas against 1019.02 for a bare
    /// wrapping unsigned one -- **+76 gas for the sign handling and the bound
    /// check**, where a second full schoolbook pass would have been on the
    /// order of +1,000.
    #[inline]
    pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
        let (mag, wide) =
            self.unsigned_abs().overflowing_mul(rhs.unsigned_abs());
        let negative = self.is_negative() != rhs.is_negative();

        let v = if negative { mag.wrapping_neg() } else { mag };

        // The bound depends on the result's sign, since the negative range
        // reaches one further.
        let limit = if negative {
            // 2^255, `MIN`'s magnitude.
            U256::ONE.wrapping_shl(255)
        } else {
            Self::MAX.cast_unsigned()
        };
        (v.cast_signed(), wide || mag.const_gt(limit))
    }

    /// Multiplication, returning `None` on overflow.
    #[inline]
    pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
        match self.overflowing_mul(rhs) {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Multiplication, wrapping on overflow.
    #[inline]
    pub const fn wrapping_mul(self, rhs: Self) -> Self {
        self.overflowing_mul(rhs).0
    }

    /// Multiplication, saturating at [`MIN`](Self::MIN) or
    /// [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_mul(self, rhs: Self) -> Self {
        match self.overflowing_mul(rhs) {
            (v, false) => v,
            (_, true) => {
                if self.is_negative() == rhs.is_negative() {
                    Self::MAX
                } else {
                    Self::MIN
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Division
    // -----------------------------------------------------------------------

    /// Divides, returning the quotient and remainder, or `None` if `rhs` is
    /// zero or the division overflows.
    ///
    /// The quotient truncates toward zero and the remainder takes the sign of
    /// the dividend, matching Rust's primitives and Solidity's `int256`.
    /// `MIN / -1` is the one overflow: its quotient is `2^255`.
    #[inline]
    pub const fn checked_div_rem(self, rhs: Self) -> Option<(Self, Self)> {
        if rhs.is_zero()
            || (self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE))
        {
            return None;
        }
        Some(self.div_rem_unchecked(rhs))
    }

    /// The division itself, with the two special cases already ruled out.
    ///
    /// `MIN / 1` still reaches here and produces a magnitude of `2^255`, which
    /// negates back to `MIN` exactly; only `MIN / -1` needs the magnitude to
    /// stay positive, and that is the case the callers reject.
    #[inline]
    const fn div_rem_unchecked(self, rhs: Self) -> (Self, Self) {
        let (q, r) =
            match self.unsigned_abs().checked_div_rem(rhs.unsigned_abs()) {
                Some(v) => v,
                None => panic!("attempt to divide by zero"),
            };
        let q = if self.is_negative() != rhs.is_negative() {
            q.wrapping_neg()
        } else {
            q
        };
        // `|r| < |rhs| <= 2^255`, so the remainder is always representable.
        let r = if self.is_negative() {
            r.wrapping_neg()
        } else {
            r
        };
        (q.cast_signed(), r.cast_signed())
    }

    /// Divides, returning the quotient and remainder.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero, or on `MIN / -1`. **Both panics are unconditional**,
    /// and deliberately so: division overflow is the one arithmetic overflow
    /// the primitives do not key off `overflow-checks`. `i128::MIN / -1`
    /// panics under `-C overflow-checks=off` exactly as it does with checks
    /// on, because the alternative on real hardware is a trap rather than a
    /// wrapped value. Matching the primitives here means *not* gating this.
    ///
    /// The consequence is worth stating plainly, since it is the opposite of
    /// what the rest of this type does: a contract profile silently wraps
    /// `MIN + 1` and `MIN * 2`, but aborts on `MIN / -1`. Reach for
    /// [`wrapping_div`](Self::wrapping_div) or
    /// [`checked_div`](Self::checked_div) if an abort is not what the call
    /// site wants.
    #[inline]
    pub const fn div_rem(self, rhs: Self) -> (Self, Self) {
        if rhs.is_zero() {
            panic!("attempt to divide by zero");
        }
        if self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE) {
            panic!("attempt to divide with overflow");
        }
        self.div_rem_unchecked(rhs)
    }

    /// Division, with a flag reporting whether it overflowed.
    ///
    /// The flag is set only by `MIN / -1`, whose wrapped quotient is `MIN`.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn overflowing_div(self, rhs: Self) -> (Self, bool) {
        if self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE) {
            return (Self::MIN, true);
        }
        (self.wrapping_div(rhs), false)
    }

    /// Remainder, with a flag reporting whether it overflowed.
    ///
    /// The flag is set only by `MIN % -1`, whose wrapped remainder is zero.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn overflowing_rem(self, rhs: Self) -> (Self, bool) {
        if self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE) {
            return (Self::ZERO, true);
        }
        (self.wrapping_rem(rhs), false)
    }

    /// Division, returning `None` if `rhs` is zero or the division overflows.
    #[inline]
    pub const fn checked_div(self, rhs: Self) -> Option<Self> {
        match self.checked_div_rem(rhs) {
            Some((q, _)) => Some(q),
            None => None,
        }
    }

    /// Remainder, returning `None` if `rhs` is zero or the division overflows.
    #[inline]
    pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
        match self.checked_div_rem(rhs) {
            Some((_, r)) => Some(r),
            None => None,
        }
    }

    /// Division, wrapping on overflow (`MIN / -1` is `MIN`).
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn wrapping_div(self, rhs: Self) -> Self {
        if self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE) {
            return Self::MIN;
        }
        self.div_rem_unchecked(rhs).0
    }

    /// Remainder, wrapping on overflow (`MIN % -1` is zero).
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn wrapping_rem(self, rhs: Self) -> Self {
        if self.const_eq(Self::MIN) && rhs.const_eq(Self::MINUS_ONE) {
            return Self::ZERO;
        }
        self.div_rem_unchecked(rhs).1
    }

    /// Euclidean division: the quotient whose remainder is never negative.
    ///
    /// `(-7).div_euclid(2)` is `-4`, where `(-7) / 2` is `-3`.
    ///
    /// # Panics
    ///
    /// As [`div_rem`](Self::div_rem).
    #[inline]
    pub const fn div_euclid(self, rhs: Self) -> Self {
        let (q, r) = self.div_rem(rhs);
        if r.is_negative() {
            // Step the quotient away from zero in the divisor's direction, so
            // the remainder becomes positive.
            if rhs.is_negative() {
                return q.wrapping_add(Self::ONE);
            }
            return q.wrapping_sub(Self::ONE);
        }
        q
    }

    /// The non-negative remainder of Euclidean division.
    ///
    /// # Panics
    ///
    /// As [`div_rem`](Self::div_rem).
    #[inline]
    pub const fn rem_euclid(self, rhs: Self) -> Self {
        let (_, r) = self.div_rem(rhs);
        if r.is_negative() {
            // `|rhs|` is at most `2^255`, whose wrapped form is `MIN`; adding
            // it to a negative `r` still lands in range.
            return r.wrapping_add(rhs.unsigned_abs().cast_signed());
        }
        r
    }

    // -----------------------------------------------------------------------
    // Shifts
    // -----------------------------------------------------------------------

    /// Left shift, masking the shift amount to `0..256`.
    ///
    /// Bit-identical to the unsigned shift: bits that fall off the top,
    /// including the sign bit, are simply gone.
    #[inline]
    pub const fn wrapping_shl(self, n: u32) -> Self {
        self.cast_unsigned().wrapping_shl(n).cast_signed()
    }

    /// Arithmetic right shift, masking the shift amount to `0..256`.
    ///
    /// The sign bit is replicated into the vacated positions, so this rounds
    /// toward negative infinity: `(-1) >> 1` is `-1`, not `MAX`.
    #[inline]
    pub const fn wrapping_shr(self, n: u32) -> Self {
        let n = n & (Self::BITS - 1);
        let shifted = self.cast_unsigned().wrapping_shr(n);
        if self.is_negative() {
            // `!(MAX >> n)` is the top `n` bits, and is zero for `n == 0` --
            // so no special case is needed there.
            let sign_fill = U256::MAX.wrapping_shr(n).const_not();
            shifted.const_bitor(sign_fill).cast_signed()
        } else {
            shifted.cast_signed()
        }
    }

    /// Left shift, masking the shift amount and reporting whether the mask
    /// changed it.
    #[inline]
    pub const fn overflowing_shl(self, n: u32) -> (Self, bool) {
        (self.wrapping_shl(n), n >= Self::BITS)
    }

    /// Arithmetic right shift, masking the shift amount and reporting whether
    /// the mask changed it.
    #[inline]
    pub const fn overflowing_shr(self, n: u32) -> (Self, bool) {
        (self.wrapping_shr(n), n >= Self::BITS)
    }

    /// Left shift, returning `None` if the shift amount is at least 256.
    #[inline]
    pub const fn checked_shl(self, n: u32) -> Option<Self> {
        if n >= Self::BITS {
            None
        } else {
            Some(self.wrapping_shl(n))
        }
    }

    /// Arithmetic right shift, returning `None` if the shift amount is at
    /// least 256.
    #[inline]
    pub const fn checked_shr(self, n: u32) -> Option<Self> {
        if n >= Self::BITS {
            None
        } else {
            Some(self.wrapping_shr(n))
        }
    }

    // -----------------------------------------------------------------------
    // Bitwise, from `const fn`
    // -----------------------------------------------------------------------

    /// Bitwise AND, usable from a `const fn`.
    #[inline]
    pub const fn const_bitand(self, rhs: Self) -> Self {
        self.cast_unsigned()
            .const_bitand(rhs.cast_unsigned())
            .cast_signed()
    }

    /// Bitwise OR, usable from a `const fn`.
    #[inline]
    pub const fn const_bitor(self, rhs: Self) -> Self {
        self.cast_unsigned()
            .const_bitor(rhs.cast_unsigned())
            .cast_signed()
    }

    /// Bitwise XOR, usable from a `const fn`.
    #[inline]
    pub const fn const_bitxor(self, rhs: Self) -> Self {
        self.cast_unsigned()
            .const_bitxor(rhs.cast_unsigned())
            .cast_signed()
    }

    /// Bitwise NOT, usable from a `const fn`. `!x` is `-x - 1`.
    #[inline]
    pub const fn const_not(self) -> Self {
        self.cast_unsigned().const_not().cast_signed()
    }

    // -----------------------------------------------------------------------
    // Exponentiation
    // -----------------------------------------------------------------------

    /// Exponentiation, with a flag reporting whether it overflowed.
    ///
    /// `x.pow(0)` is one for every `x`, matching the primitives.
    pub const fn overflowing_pow(self, exp: u32) -> (Self, bool) {
        let mut base = self;
        let mut exp = exp;
        let mut acc = Self::ONE;
        let mut overflow = false;

        while exp > 0 {
            if exp & 1 == 1 {
                let (v, o) = acc.overflowing_mul(base);
                acc = v;
                overflow |= o;
            }
            exp >>= 1;
            if exp > 0 {
                let (v, o) = base.overflowing_mul(base);
                base = v;
                overflow |= o;
            }
        }

        (acc, overflow)
    }

    /// Exponentiation, returning `None` on overflow.
    #[inline]
    pub const fn checked_pow(self, exp: u32) -> Option<Self> {
        match self.overflowing_pow(exp) {
            (v, false) => Some(v),
            (_, true) => None,
        }
    }

    /// Exponentiation, wrapping on overflow.
    #[inline]
    pub const fn wrapping_pow(self, exp: u32) -> Self {
        self.overflowing_pow(exp).0
    }

    /// Exponentiation, saturating at [`MIN`](Self::MIN) or
    /// [`MAX`](Self::MAX).
    #[inline]
    pub const fn saturating_pow(self, exp: u32) -> Self {
        match self.overflowing_pow(exp) {
            (v, false) => v,
            // An overflowing power is negative exactly when the base is
            // negative and the exponent odd.
            (_, true) => {
                if self.is_negative() && exp % 2 == 1 {
                    Self::MIN
                } else {
                    Self::MAX
                }
            }
        }
    }

    /// Exponentiation.
    ///
    /// # Panics
    ///
    /// On overflow, if `overflow-checks` is on -- otherwise it wraps.
    #[inline]
    pub const fn pow(self, exp: u32) -> Self {
        let (v, overflow) = self.overflowing_pow(exp);
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to multiply with overflow");
        }
        v
    }
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

impl Add for I256 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_add(rhs);
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to add with overflow");
        }
        v
    }
}

impl Sub for I256 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_sub(rhs);
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to subtract with overflow");
        }
        v
    }
}

impl Mul for I256 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_mul(rhs);
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to multiply with overflow");
        }
        v
    }
}

impl Div for I256 {
    type Output = Self;

    /// Truncating division. Panics on a zero divisor regardless of
    /// `overflow-checks`, matching the primitive integers.
    #[inline]
    fn div(self, rhs: Self) -> Self {
        self.div_rem(rhs).0
    }
}

impl Rem for I256 {
    type Output = Self;

    /// Remainder, taking the sign of the dividend. Panics on a zero divisor
    /// regardless of `overflow-checks`, matching the primitive integers.
    #[inline]
    fn rem(self, rhs: Self) -> Self {
        self.div_rem(rhs).1
    }
}

impl Neg for I256 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        let (v, overflow) = self.overflowing_neg();
        if OVERFLOW_CHECKS && overflow {
            panic!("attempt to negate with overflow");
        }
        v
    }
}

impl Not for I256 {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        self.const_not()
    }
}

macro_rules! impl_assign {
    ($trait:ident, $method:ident, $base:ident, $base_method:ident, $rhs:ty) => {
        impl $trait<$rhs> for I256 {
            #[inline]
            fn $method(&mut self, rhs: $rhs) {
                *self = <Self as $base<$rhs>>::$base_method(*self, rhs);
            }
        }
    };
}

/// Shifts take every integer type, for the reason the unsigned side does: with
/// a single `Shl<u32>` impl a bare `x << 112` builds, but a second impl makes
/// the literal ambiguous and it falls back to `i32`.
macro_rules! impl_shift {
    ($($t:ty),* $(,)?) => {
        $(
            impl Shl<$t> for I256 {
                type Output = Self;

                #[inline]
                fn shl(self, rhs: $t) -> Self {
                    let (bits, overflow) = crate::arith::shift_bits(rhs as u128);
                    if OVERFLOW_CHECKS && overflow {
                        panic!("attempt to shift left with overflow");
                    }
                    self.wrapping_shl(bits)
                }
            }

            impl Shr<$t> for I256 {
                type Output = Self;

                #[inline]
                fn shr(self, rhs: $t) -> Self {
                    let (bits, overflow) = crate::arith::shift_bits(rhs as u128);
                    if OVERFLOW_CHECKS && overflow {
                        panic!("attempt to shift right with overflow");
                    }
                    self.wrapping_shr(bits)
                }
            }

            impl_assign!(ShlAssign, shl_assign, Shl, shl, $t);
            impl_assign!(ShrAssign, shr_assign, Shr, shr, $t);
        )*
    };
}

impl_shift!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

macro_rules! impl_bitop {
    ($trait:ident, $method:ident, $const_method:ident) => {
        impl $trait for I256 {
            type Output = Self;

            #[inline]
            fn $method(self, rhs: Self) -> Self {
                self.$const_method(rhs)
            }
        }
    };
}

impl_bitop!(BitAnd, bitand, const_bitand);
impl_bitop!(BitOr, bitor, const_bitor);
impl_bitop!(BitXor, bitxor, const_bitxor);

impl_assign!(AddAssign, add_assign, Add, add, I256);
impl_assign!(SubAssign, sub_assign, Sub, sub, I256);
impl_assign!(MulAssign, mul_assign, Mul, mul, I256);
impl_assign!(DivAssign, div_assign, Div, div, I256);
impl_assign!(RemAssign, rem_assign, Rem, rem, I256);
impl_assign!(BitAndAssign, bitand_assign, BitAnd, bitand, I256);
impl_assign!(BitOrAssign, bitor_assign, BitOr, bitor, I256);
impl_assign!(BitXorAssign, bitxor_assign, BitXor, bitxor, I256);
