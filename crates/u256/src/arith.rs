//! Addition, subtraction, multiplication, shifts and bitwise operations.
//!
//! The operators mirror the primitive integers exactly: they wrap when
//! `overflow-checks` is off and panic when it is on. Every `panic!` here is
//! guarded by `cfg!(overflow_checks)`, which const-folds to `false` in a
//! release contract build, so the branch and its message are eliminated.

use crate::{LIMBS, U256};
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor,
    BitXorAssign, Div, DivAssign, Mul, MulAssign, Not, Rem, RemAssign, Shl,
    ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

/// `acc + a * b + carry`, split into (low, high) 32-bit halves.
///
/// The widest possible value is `(2^32 - 1)^2 + 2 * (2^32 - 1) = 2^64 - 1`, so
/// the `u64` intermediate never overflows. On rv32 this is a `mul`/`mulhu`
/// pair; on wasm32 an `i64.extend_u` and an `i64.mul`. No libcall either way.
#[inline(always)]
const fn mac(acc: u32, a: u32, b: u32, carry: u32) -> (u32, u32) {
    let t = (a as u64) * (b as u64) + (acc as u64) + (carry as u64);
    (t as u32, (t >> 32) as u32)
}

impl U256 {
    // -----------------------------------------------------------------------
    // Addition and subtraction
    // -----------------------------------------------------------------------

    /// Wrapping addition, with a flag reporting whether it wrapped.
    #[inline]
    pub const fn overflowing_add(self, rhs: Self) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut carry = 0u32;
        let mut i = 0;
        while i < LIMBS {
            let t = (self.0[i] as u64) + (rhs.0[i] as u64) + (carry as u64);
            out[i] = t as u32;
            carry = (t >> 32) as u32;
            i += 1;
        }
        (Self(out), carry != 0)
    }

    /// Wrapping subtraction, with a flag reporting whether it wrapped.
    #[inline]
    pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut borrow = 0u32;
        let mut i = 0;
        while i < LIMBS {
            let t = (self.0[i] as u64)
                .wrapping_sub(rhs.0[i] as u64)
                .wrapping_sub(borrow as u64);
            out[i] = t as u32;
            borrow = ((t >> 32) & 1) as u32;
            i += 1;
        }
        (Self(out), borrow != 0)
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

    /// Addition, saturating at [`U256::MAX`].
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        match self.overflowing_add(rhs) {
            (v, false) => v,
            (_, true) => Self::MAX,
        }
    }

    /// Subtraction, saturating at [`U256::ZERO`].
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        match self.overflowing_sub(rhs) {
            (v, false) => v,
            (_, true) => Self::ZERO,
        }
    }

    // -----------------------------------------------------------------------
    // Multiplication
    // -----------------------------------------------------------------------

    /// The exact 256-bit product of two 128-bit values.
    ///
    /// This is the shape that dominates real call sites -- reserve products,
    /// LP-share ratios, any `a * b / c` where `a` and `b` are token amounts.
    /// It cannot overflow, so it needs no check and no `Option`, and it costs
    /// sixteen `mul`/`mulhu` pairs with no carry-propagation loop over the
    /// upper half.
    #[inline]
    pub const fn widening_mul(a: u128, b: u128) -> Self {
        let al = [
            a as u32,
            (a >> 32) as u32,
            (a >> 64) as u32,
            (a >> 96) as u32,
        ];
        let bl = [
            b as u32,
            (b >> 32) as u32,
            (b >> 64) as u32,
            (b >> 96) as u32,
        ];

        let mut out = [0u32; LIMBS];
        let mut i = 0;
        while i < 4 {
            let mut carry = 0u32;
            let mut j = 0;
            while j < 4 {
                let (lo, hi) = mac(out[i + j], al[i], bl[j], carry);
                out[i + j] = lo;
                carry = hi;
                j += 1;
            }
            out[i + 4] = carry;
            i += 1;
        }
        Self(out)
    }

    /// Multiplication by a 32-bit value, with a flag reporting overflow.
    ///
    /// Deliberately separate from the general 256x256 multiply: scaling by a
    /// small constant (fee numerators, basis points) is common, and should not
    /// drag the full schoolbook loop into a contract that never needs it.
    #[inline]
    pub const fn overflowing_mul_u32(self, rhs: u32) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut carry = 0u32;
        let mut i = 0;
        while i < LIMBS {
            let (lo, hi) = mac(0, self.0[i], rhs, carry);
            out[i] = lo;
            carry = hi;
            i += 1;
        }
        (Self(out), carry != 0)
    }

    /// Wrapping multiplication, with a flag reporting whether it wrapped.
    #[inline]
    pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut overflow = false;

        let mut i = 0;
        while i < LIMBS {
            let mut carry = 0u32;
            let mut j = 0;
            // Products landing inside the low 256 bits.
            while j < LIMBS - i {
                let (lo, hi) = mac(out[i + j], self.0[i], rhs.0[j], carry);
                out[i + j] = lo;
                carry = hi;
                j += 1;
            }
            // A carry out of the top limb is a lost bit.
            if carry != 0 {
                overflow = true;
            }
            // So is any product whose position is past the top limb.
            while j < LIMBS {
                if self.0[i] != 0 && rhs.0[j] != 0 {
                    overflow = true;
                }
                j += 1;
            }
            i += 1;
        }

        (Self(out), overflow)
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

    /// Multiplication, saturating at [`U256::MAX`].
    #[inline]
    pub const fn saturating_mul(self, rhs: Self) -> Self {
        match self.overflowing_mul(rhs) {
            (v, false) => v,
            (_, true) => Self::MAX,
        }
    }

    // -----------------------------------------------------------------------
    // Shifts
    // -----------------------------------------------------------------------

    /// Left shift, masking the shift amount to `0..256` and reporting whether
    /// the mask changed it. Mirrors `u128::overflowing_shl`.
    #[inline]
    pub const fn overflowing_shl(self, n: u32) -> (Self, bool) {
        (self.wrapping_shl(n), n >= Self::BITS)
    }

    /// Left shift, masking the shift amount to `0..256`.
    #[inline]
    pub const fn wrapping_shl(self, n: u32) -> Self {
        let n = n & (Self::BITS - 1);
        let limb_shift = (n / 32) as usize;
        let bit_shift = n % 32;

        let mut out = [0u32; LIMBS];
        let mut i = LIMBS;
        while i > limb_shift {
            i -= 1;
            let src = i - limb_shift;
            let mut v = self.0[src] << bit_shift;
            if bit_shift > 0 && src > 0 {
                v |= self.0[src - 1] >> (32 - bit_shift);
            }
            out[i] = v;
        }
        Self(out)
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

    /// Right shift, masking the shift amount to `0..256` and reporting whether
    /// the mask changed it. Mirrors `u128::overflowing_shr`.
    #[inline]
    pub const fn overflowing_shr(self, n: u32) -> (Self, bool) {
        (self.wrapping_shr(n), n >= Self::BITS)
    }

    /// Right shift, masking the shift amount to `0..256`.
    #[inline]
    pub const fn wrapping_shr(self, n: u32) -> Self {
        let n = n & (Self::BITS - 1);
        let limb_shift = (n / 32) as usize;
        let bit_shift = n % 32;

        let mut out = [0u32; LIMBS];
        let mut i = 0;
        while i + limb_shift < LIMBS {
            let src = i + limb_shift;
            let mut v = self.0[src] >> bit_shift;
            if bit_shift > 0 && src + 1 < LIMBS {
                v |= self.0[src + 1] << (32 - bit_shift);
            }
            out[i] = v;
            i += 1;
        }
        Self(out)
    }

    /// Right shift, returning `None` if the shift amount is at least 256.
    #[inline]
    pub const fn checked_shr(self, n: u32) -> Option<Self> {
        if n >= Self::BITS {
            None
        } else {
            Some(self.wrapping_shr(n))
        }
    }
}

// ---------------------------------------------------------------------------
// Operators
//
// `cfg!(overflow_checks)` is a compile-time bool, so in a release contract
// build (`overflow-checks = false`) each guard folds to `false` and the panic
// is eliminated along with its message.
// ---------------------------------------------------------------------------

impl Add for U256 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_add(rhs);
        if cfg!(overflow_checks) && overflow {
            panic!("attempt to add with overflow");
        }
        v
    }
}

impl Sub for U256 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_sub(rhs);
        if cfg!(overflow_checks) && overflow {
            panic!("attempt to subtract with overflow");
        }
        v
    }
}

impl Mul for U256 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let (v, overflow) = self.overflowing_mul(rhs);
        if cfg!(overflow_checks) && overflow {
            panic!("attempt to multiply with overflow");
        }
        v
    }
}

impl Mul<u32> for U256 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u32) -> Self {
        let (v, overflow) = self.overflowing_mul_u32(rhs);
        if cfg!(overflow_checks) && overflow {
            panic!("attempt to multiply with overflow");
        }
        v
    }
}

impl Div for U256 {
    type Output = Self;

    /// Division. Panics on a zero divisor regardless of `overflow-checks`,
    /// matching the primitive integers.
    #[inline]
    fn div(self, rhs: Self) -> Self {
        self.div_rem(rhs).0
    }
}

impl Rem for U256 {
    type Output = Self;

    /// Remainder. Panics on a zero divisor regardless of `overflow-checks`,
    /// matching the primitive integers.
    #[inline]
    fn rem(self, rhs: Self) -> Self {
        self.div_rem(rhs).1
    }
}

impl Shl<u32> for U256 {
    type Output = Self;

    #[inline]
    fn shl(self, rhs: u32) -> Self {
        if cfg!(overflow_checks) && rhs >= Self::BITS {
            panic!("attempt to shift left with overflow");
        }
        self.wrapping_shl(rhs)
    }
}

impl Shr<u32> for U256 {
    type Output = Self;

    #[inline]
    fn shr(self, rhs: u32) -> Self {
        if cfg!(overflow_checks) && rhs >= Self::BITS {
            panic!("attempt to shift right with overflow");
        }
        self.wrapping_shr(rhs)
    }
}

impl Not for U256 {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        let mut out = [0u32; LIMBS];
        let mut i = 0;
        while i < LIMBS {
            out[i] = !self.0[i];
            i += 1;
        }
        Self(out)
    }
}

macro_rules! impl_bitop {
    ($trait:ident, $method:ident, $op:tt) => {
        impl $trait for U256 {
            type Output = Self;

            #[inline]
            fn $method(self, rhs: Self) -> Self {
                let mut out = [0u32; LIMBS];
                let mut i = 0;
                while i < LIMBS {
                    out[i] = self.0[i] $op rhs.0[i];
                    i += 1;
                }
                Self(out)
            }
        }
    };
}

impl_bitop!(BitAnd, bitand, &);
impl_bitop!(BitOr, bitor, |);
impl_bitop!(BitXor, bitxor, ^);

macro_rules! impl_assign {
    ($trait:ident, $method:ident, $base:ident, $base_method:ident, $rhs:ty) => {
        impl $trait<$rhs> for U256 {
            #[inline]
            fn $method(&mut self, rhs: $rhs) {
                *self = <Self as $base<$rhs>>::$base_method(*self, rhs);
            }
        }
    };
}

impl_assign!(AddAssign, add_assign, Add, add, U256);
impl_assign!(SubAssign, sub_assign, Sub, sub, U256);
impl_assign!(MulAssign, mul_assign, Mul, mul, U256);
impl_assign!(MulAssign, mul_assign, Mul, mul, u32);
impl_assign!(DivAssign, div_assign, Div, div, U256);
impl_assign!(RemAssign, rem_assign, Rem, rem, U256);
impl_assign!(BitAndAssign, bitand_assign, BitAnd, bitand, U256);
impl_assign!(BitOrAssign, bitor_assign, BitOr, bitor, U256);
impl_assign!(BitXorAssign, bitxor_assign, BitXor, bitxor, U256);
impl_assign!(ShlAssign, shl_assign, Shl, shl, u32);
impl_assign!(ShrAssign, shr_assign, Shr, shr, u32);
