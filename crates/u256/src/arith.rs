//! Addition, subtraction, multiplication, shifts and bitwise operations.
//!
//! The operators mirror the primitives: wrap when `overflow-checks` is off,
//! panic when it is on. Each `panic!` is guarded by [`OVERFLOW_CHECKS`],
//! which const-folds away in a release contract build.
//!
//! # Inlining
//!
//! The big bodies here are `#[inline(never)]` on rv32 and `#[inline]`
//! everywhere else. Two things drive that split.
//!
//! Deleting `#[inline]` would be a no-op: contract profiles build with fat LTO
//! and one codegen unit, where LLVM inlines on its own cost model regardless
//! of the hint. Only `inline(never)` changes anything.
//!
//! And the right answer differs by target, because inlining lets LLVM
//! specialize each copy against known-zero limbs. Measured on rv32 `.text` and
//! wasm `CODE` against a probe with N distinct call sites, `inline(never)`
//! breaks even at roughly:
//!
//! | function                          | rv32 | wasm |
//! |-----------------------------------|------|------|
//! | `widening_mul`, `overflowing_mul` | 1.3  | 5.0  |
//! | `from_be_bytes`                   | 1.2  | 2.2  |
//! | `to_be_bytes`                     | 1.8  | 18   |
//!
//! Real contracts sit at a handful of call sites (the swap pair has four
//! multiply sites), so outlining pays on rv32 and costs on wasm. rv32 is where
//! the 64 KiB artifact cap binds, hence the `cfg_attr`. Keep the small bodies
//! -- add/sub/shift/compare/`is_zero`/`from_limbs` -- plain `#[inline]`: they
//! are smaller than the call sequence that would replace them.

use crate::{LIMBS, U256};
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor,
    BitXorAssign, Div, DivAssign, Mul, MulAssign, Not, Rem, RemAssign, Shl,
    ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

/// `acc + a * b + carry`, split into (low, high) 32-bit halves.
///
/// Peaks at `(2^32-1)^2 + 2*(2^32-1) = 2^64-1`, so the `u64` never overflows.
/// A `mul`/`mulhu` pair on rv32, `i64.extend_u`/`i64.mul` on wasm32.
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
            let t = (self.limbs[i] as u64)
                + (rhs.limbs[i] as u64)
                + (carry as u64);
            out[i] = t as u32;
            carry = (t >> 32) as u32;
            i += 1;
        }
        (Self::from_limbs(out), carry != 0)
    }

    /// Wrapping subtraction, with a flag reporting whether it wrapped.
    #[inline]
    pub const fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut borrow = 0u32;
        let mut i = 0;
        while i < LIMBS {
            let t = (self.limbs[i] as u64)
                .wrapping_sub(rhs.limbs[i] as u64)
                .wrapping_sub(borrow as u64);
            out[i] = t as u32;
            borrow = ((t >> 32) & 1) as u32;
            i += 1;
        }
        (Self::from_limbs(out), borrow != 0)
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
    /// The shape that dominates real call sites (`a * b / c` over token
    /// amounts). Cannot overflow, so no check and no `Option`; sixteen
    /// `mul`/`mulhu` pairs.
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(not(target_arch = "riscv32"), inline)]
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
        Self::from_limbs(out)
    }

    /// Multiplication by a 32-bit value, with a flag reporting overflow.
    ///
    /// Separate from the general 256x256 multiply so scaling by a small
    /// constant does not drag the schoolbook loop in.
    #[inline]
    pub const fn overflowing_mul_u32(self, rhs: u32) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut carry = 0u32;
        let mut i = 0;
        while i < LIMBS {
            let (lo, hi) = mac(0, self.limbs[i], rhs, carry);
            out[i] = lo;
            carry = hi;
            i += 1;
        }
        (Self::from_limbs(out), carry != 0)
    }

    /// Wrapping multiplication, with a flag reporting whether it wrapped.
    ///
    /// Outlined on rv32; see [`U256::widening_mul`] and the note at the top of
    /// this module.
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    #[cfg_attr(not(target_arch = "riscv32"), inline)]
    pub const fn overflowing_mul(self, rhs: Self) -> (Self, bool) {
        let mut out = [0u32; LIMBS];
        let mut overflow = false;

        let mut i = 0;
        while i < LIMBS {
            let mut carry = 0u32;
            let mut j = 0;
            // Products landing inside the low 256 bits.
            while j < LIMBS - i {
                let (lo, hi) =
                    mac(out[i + j], self.limbs[i], rhs.limbs[j], carry);
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
                if self.limbs[i] != 0 && rhs.limbs[j] != 0 {
                    overflow = true;
                }
                j += 1;
            }
            i += 1;
        }

        (Self::from_limbs(out), overflow)
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
            let mut v = self.limbs[src] << bit_shift;
            if bit_shift > 0 && src > 0 {
                v |= self.limbs[src - 1] >> (32 - bit_shift);
            }
            out[i] = v;
        }
        Self::from_limbs(out)
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
            let mut v = self.limbs[src] >> bit_shift;
            if bit_shift > 0 && src + 1 < LIMBS {
                v |= self.limbs[src + 1] << (32 - bit_shift);
            }
            out[i] = v;
            i += 1;
        }
        Self::from_limbs(out)
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
// `OVERFLOW_CHECKS` is a compile-time bool, so in a release contract build
// (`overflow-checks = false`) each guard folds to `false` and the panic is
// eliminated along with its message.
// ---------------------------------------------------------------------------

// With the `nightly` feature this is the same flag rustc keys the primitives'
// operators off; the stable fallback reads `debug_assertions`, which every
// profile in this workspace moves in lockstep with `overflow-checks` (see the
// gate in lib.rs).
#[cfg(feature = "nightly")]
const OVERFLOW_CHECKS: bool = cfg!(overflow_checks);
#[cfg(not(feature = "nightly"))]
const OVERFLOW_CHECKS: bool = cfg!(debug_assertions);

impl Add for U256 {
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

impl Sub for U256 {
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

impl Mul for U256 {
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

impl Mul<u32> for U256 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u32) -> Self {
        let (v, overflow) = self.overflowing_mul_u32(rhs);
        if OVERFLOW_CHECKS && overflow {
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

/// Splits a shift amount, already widened to `u128`, into the amount the shift
/// primitives take and whether it was out of range.
///
/// Masking, not saturating. The primitives reduce an out-of-range shift modulo
/// the width, so the amount that survives is the low `log2(BITS)` bits of the
/// operand: `1u64 << 32` shifts by `(1 << 32) % 256 == 0`, and `-2` shifts by
/// 254. Collapsing those to `u32::MAX` would shift by 255 in both cases, which
/// is wrong in exactly the profiles that matter -- with `overflow-checks` off,
/// the mask is the whole behaviour and nothing panics to hide it.
///
/// Callers widen with `as`, which sign-extends, so a negative amount arrives
/// here as a very large `u128`: out of range for the panic path, and masking to
/// its own low bits for the wrapping path, both matching the primitives.
#[inline]
const fn shift_bits(wide: u128) -> (u32, bool) {
    ((wide as u32) & (U256::BITS - 1), wide >= U256::BITS as u128)
}

// Defined before `impl_shift!`, which expands it: `macro_rules!` scoping is
// textual, not item-order-independent. The invocations for the non-shift
// operators stay with the bitwise impls further down.
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

/// Shifts are implemented for every integer type, exactly as the primitives do
/// it.
///
/// The full family is load-bearing, not a convenience: with a single
/// `Shl<u32>` impl an unsuffixed `x << 112` resolves the literal to `u32` and
/// builds, but as soon as a second impl exists the literal becomes ambiguous
/// and falls back to `i32`. Omitting the signed types here would break every
/// bare-literal shift in the wild.
macro_rules! impl_shift {
    ($($t:ty),* $(,)?) => {
        $(
            impl Shl<$t> for U256 {
                type Output = Self;

                #[inline]
                fn shl(self, rhs: $t) -> Self {
                    let (bits, overflow) = shift_bits(rhs as u128);
                    if OVERFLOW_CHECKS && overflow {
                        panic!("attempt to shift left with overflow");
                    }
                    self.wrapping_shl(bits)
                }
            }

            impl Shr<$t> for U256 {
                type Output = Self;

                #[inline]
                fn shr(self, rhs: $t) -> Self {
                    let (bits, overflow) = shift_bits(rhs as u128);
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

impl Not for U256 {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        let mut out = [0u32; LIMBS];
        let mut i = 0;
        while i < LIMBS {
            out[i] = !self.limbs[i];
            i += 1;
        }
        Self::from_limbs(out)
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
                    out[i] = self.limbs[i] $op rhs.limbs[i];
                    i += 1;
                }
                Self::from_limbs(out)
            }
        }
    };
}

impl_bitop!(BitAnd, bitand, &);
impl_bitop!(BitOr, bitor, |);
impl_bitop!(BitXor, bitxor, ^);

// `impl_assign!` itself is defined above `impl_shift!`, which also expands it.
impl_assign!(AddAssign, add_assign, Add, add, U256);
impl_assign!(SubAssign, sub_assign, Sub, sub, U256);
impl_assign!(MulAssign, mul_assign, Mul, mul, U256);
impl_assign!(MulAssign, mul_assign, Mul, mul, u32);
impl_assign!(DivAssign, div_assign, Div, div, U256);
impl_assign!(RemAssign, rem_assign, Rem, rem, U256);
impl_assign!(BitAndAssign, bitand_assign, BitAnd, bitand, U256);
impl_assign!(BitOrAssign, bitor_assign, BitOr, bitor, U256);
impl_assign!(BitXorAssign, bitxor_assign, BitXor, bitxor, U256);

#[cfg(test)]
mod tests {
    use super::shift_bits;

    /// The operator-level version of this can only be tested in one profile at
    /// a time -- with `overflow-checks` on every out-of-range shift panics
    /// before the mask is observable, and that is the profile tests run in.
    /// The mask is the entire behaviour of a release contract build, so it is
    /// pinned here instead.
    #[test]
    fn out_of_range_shift_amounts_mask_rather_than_saturate() {
        assert_eq!(shift_bits(0), (0, false));
        assert_eq!(shift_bits(255), (255, false));
        assert_eq!(shift_bits(256), (0, true));
        // 300 % 256; saturating to `u32::MAX` would have said 255.
        assert_eq!(shift_bits(300), (44, true));
        // Wider than `u32`: the low bits survive the narrowing.
        assert_eq!(shift_bits(1u128 << 32), (0, true));
        assert_eq!(shift_bits((1u128 << 32) + 7), (7, true));
    }

    /// Callers widen with `as`, so negatives arrive sign-extended.
    #[test]
    fn negative_shift_amounts_mask_to_their_low_bits() {
        assert_eq!(shift_bits(-1i32 as u128), (255, true));
        // Saturating gave 255 here too, which is the bug: -2 is 254.
        assert_eq!(shift_bits(-2i32 as u128), (254, true));
        assert_eq!(shift_bits(-256i64 as u128), (0, true));
    }
}
