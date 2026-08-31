//! A 256-bit signed integer, two's complement over the same `[u32; 8]`.
//!
//! [`I256`] is [`U256`]'s bit pattern read as a signed value, and that is not
//! just an implementation note: addition, subtraction, multiplication and the
//! bitwise operations are *the same instructions* on both types, so the signed
//! half of this crate costs almost nothing beyond the operations where the
//! sign genuinely changes the answer -- comparison, division, right shift,
//! and formatting.
//!
//! Semantics follow Rust's primitives, which on every point that matters are
//! also Solidity's:
//!
//! * Division truncates toward zero, and the remainder takes the sign of the
//!   dividend (`-7 / 2 == -3`, `-7 % 2 == -1`).
//! * `>>` is an arithmetic shift: the sign bit is replicated.
//! * `MIN.abs()` and `-MIN` overflow. Like the operators on [`U256`], they
//!   panic when `overflow-checks` is on and wrap when it is off; the
//!   `checked_` forms report instead.
//! * `MIN / -1` and `MIN % -1` overflow too, but they **panic
//!   unconditionally**, in a release contract profile as well. That is not an
//!   inconsistency: division overflow is the one case the primitives also
//!   leave ungated, because on real hardware the alternative is a trap rather
//!   than a wrapped value. `i128::MIN / -1` panics under
//!   `-C overflow-checks=off` too. Use `wrapping_div`/`checked_div` where an
//!   abort is not wanted.
//!
//! The wire encoding is the two's complement value as 32 big-endian bytes --
//! the same 32 bytes a `U256` with those bits would produce, and what
//! Ethereum's `int256` uses.
//!
//! # Where the operators differ from Solidity
//!
//! The *library functions* match OpenZeppelin exactly, degenerate inputs
//! included -- `tests/oz_parity.rs` pins that case by case. The *operators*
//! follow Rust, and on two points Rust and Solidity genuinely disagree. A port
//! should know which:
//!
//! | | Solidity (checked, the default) | Solidity `unchecked` | Rust, and here |
//! |---|---|---|---|
//! | `x / 0`, `x % 0` | revert | revert (not disableable) | panic |
//! | `MIN / -1` | revert | wraps to `MIN` | panic |
//! | `MIN % -1` | `0` | `0` | **panic** |
//!
//! `MIN / -1` agrees with Solidity's default, which is the one that matters:
//! `unchecked` is opt-in per block in Solidity, whereas `overflow-checks` here
//! is a whole-build setting nobody wrote at the call site, so treating the two
//! as equivalent would silently produce `MIN` where the ported original
//! reverted.
//!
//! `MIN % -1` is the real divergence. Solidity's `smod` is a separate opcode
//! that simply answers 0, and 0 is the mathematically correct remainder;
//! Rust panics because x86 computes quotient and remainder with one
//! instruction that traps on both. We follow Rust so that `I256` and the
//! `i128` beside it in the same expression behave alike -- the rule the rest
//! of this type's overflow behaviour is built on. Use
//! [`wrapping_rem`](I256::wrapping_rem), which answers 0, if a port depends
//! on the Solidity result.

use crate::{LIMBS, TryFromIntError, U256};

/// A 256-bit signed integer.
///
/// Limbs are the two's complement representation, least-significant-first, so
/// the layout and the wire encoding match [`U256`] exactly. See the module
/// documentation for the semantics.
///
/// Named field, and named `limbs`, for the reason [`U256`] gives: the spec
/// derive would call an unnamed field `_0`, and the field name is
/// wire-visible.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    groundhog_macros::ContractTypeSpec,
)]
#[contract_type_spec(crate = "crate::__sdk_paths")]
pub struct I256 {
    pub(crate) limbs: [u32; LIMBS],
}

impl I256 {
    /// The additive identity.
    pub const ZERO: Self = Self::from_limbs([0; LIMBS]);

    /// The multiplicative identity.
    pub const ONE: Self = Self::from_limbs([1, 0, 0, 0, 0, 0, 0, 0]);

    /// Negative one, whose two's complement is all ones.
    pub const MINUS_ONE: Self = Self::from_limbs([u32::MAX; LIMBS]);

    /// The largest representable value, `2^255 - 1`.
    pub const MAX: Self = U256::MAX.wrapping_shr(1).cast_signed();

    /// The smallest representable value, `-2^255`.
    ///
    /// Its magnitude is one larger than [`MAX`](Self::MAX)'s, which is why
    /// negating, absolute value and division by `-1` can all overflow.
    pub const MIN: Self = U256::ONE.wrapping_shl(255).cast_signed();

    /// The size of this integer in bits, sign bit included.
    pub const BITS: u32 = 256;

    /// Constructs a value from its two's complement limbs,
    /// least-significant-first.
    #[inline]
    pub const fn from_limbs(limbs: [u32; LIMBS]) -> Self {
        Self { limbs }
    }

    /// Returns the two's complement limbs, least-significant-first.
    #[inline]
    pub const fn to_limbs(self) -> [u32; LIMBS] {
        self.limbs
    }

    /// Constructs a value from an `i128`.
    ///
    /// Inherent for the reason [`U256::from`] is: `From` is implemented for
    /// every signed width, which would leave `I256::from(-1000)` ambiguous,
    /// and an inherent function shadows the trait in path form and pins the
    /// literal to `i128`. It is also usable in const position.
    ///
    /// The trait impls are untouched, so `.into()` and generic `T: Into<I256>`
    /// still convert from any width -- including the unsigned ones, which all
    /// fit.
    #[inline]
    pub const fn from(n: i128) -> Self {
        Self::from_i128(n)
    }

    /// Constructs a value from an `i128`, under a name no trait shadows.
    #[inline]
    pub const fn from_i128(n: i128) -> Self {
        // Sign-extend: the upper limbs are all ones for a negative value.
        let fill = if n < 0 { u32::MAX } else { 0 };
        Self::from_limbs([
            n as u32,
            (n >> 32) as u32,
            (n >> 64) as u32,
            (n >> 96) as u32,
            fill,
            fill,
            fill,
            fill,
        ])
    }

    /// Constructs a value from an `i32`, without widening at the call site.
    #[inline]
    pub const fn from_i32(n: i32) -> Self {
        let fill = if n < 0 { u32::MAX } else { 0 };
        Self::from_limbs([n as u32, fill, fill, fill, fill, fill, fill, fill])
    }

    /// Constructs a value from an `i64`, without widening at the call site.
    #[inline]
    pub const fn from_i64(n: i64) -> Self {
        let fill = if n < 0 { u32::MAX } else { 0 };
        Self::from_limbs([
            n as u32,
            (n >> 32) as u32,
            fill,
            fill,
            fill,
            fill,
            fill,
            fill,
        ])
    }

    /// Reinterprets the two's complement bits as a [`U256`].
    ///
    /// No arithmetic: `(-1).cast_unsigned()` is [`U256::MAX`]. Named after
    /// `i128::cast_unsigned`, which does the same thing at a narrower width.
    #[inline]
    pub const fn cast_unsigned(self) -> U256 {
        U256::from_limbs(self.limbs)
    }

    /// Returns `true` if this is zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.cast_unsigned().is_zero()
    }

    /// Returns `true` if this is strictly less than zero.
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.limbs[LIMBS - 1] >> 31 == 1
    }

    /// Returns `true` if this is strictly greater than zero.
    #[inline]
    pub const fn is_positive(self) -> bool {
        !self.is_negative() && !self.is_zero()
    }

    /// `-1`, `0` or `1`, according to the sign.
    #[inline]
    pub const fn signum(self) -> Self {
        if self.is_zero() {
            Self::ZERO
        } else if self.is_negative() {
            Self::MINUS_ONE
        } else {
            Self::ONE
        }
    }

    /// The absolute value, as a [`U256`].
    ///
    /// Total, unlike [`abs`](Self::abs): `MIN.unsigned_abs()` is `2^255`,
    /// which a `U256` holds comfortably. `SignedMath.abs`.
    #[inline]
    pub const fn unsigned_abs(self) -> U256 {
        if self.is_negative() {
            self.cast_unsigned().wrapping_neg()
        } else {
            self.cast_unsigned()
        }
    }

    /// Returns the bit at `i`, counting from the least significant. Bit 255 is
    /// the sign bit.
    ///
    /// Bits at or above [`BITS`](Self::BITS) read as zero rather than
    /// panicking, matching [`U256::bit`] -- note that this is *not* a sign
    /// extension.
    #[inline]
    pub const fn bit(self, i: u32) -> bool {
        self.cast_unsigned().bit(i)
    }

    /// Comparison usable from a `const fn`.
    ///
    /// Flipping the sign bit maps the signed order onto the unsigned one, so
    /// this is [`U256::const_cmp`] on the flipped values.
    #[inline]
    pub const fn const_cmp(self, other: Self) -> core::cmp::Ordering {
        let flip = U256::ONE.wrapping_shl(255);
        self.cast_unsigned()
            .const_bitxor(flip)
            .const_cmp(other.cast_unsigned().const_bitxor(flip))
    }

    /// `self == other`, from a `const fn`.
    #[inline]
    pub(crate) const fn const_eq(self, other: Self) -> bool {
        self.cast_unsigned().const_eq(other.cast_unsigned())
    }

    /// `self < other`, from a `const fn`.
    #[inline]
    pub(crate) const fn const_lt(self, other: Self) -> bool {
        matches!(self.const_cmp(other), core::cmp::Ordering::Less)
    }

    /// `self > other`, from a `const fn`.
    #[inline]
    pub(crate) const fn const_gt(self, other: Self) -> bool {
        matches!(self.const_cmp(other), core::cmp::Ordering::Greater)
    }

    /// Returns the big-endian two's complement byte representation.
    ///
    /// `#[inline]`, not the `inline(never)` the unsigned pair carries: the
    /// body is a reinterpretation and a call, so the byte-swap loop this
    /// reaches is [`U256::to_be_bytes`], which is *already* outlined on rv32
    /// for exactly that reason. Outlining the wrapper too would add a second
    /// symbol and a call/return per conversion, not remove one.
    #[inline]
    pub const fn to_be_bytes(self) -> [u8; 32] {
        self.cast_unsigned().to_be_bytes()
    }

    /// Constructs a value from its big-endian two's complement byte
    /// representation.
    ///
    /// `#[inline]` for the reason [`to_be_bytes`](Self::to_be_bytes) gives.
    #[inline]
    pub const fn from_be_bytes(bytes: [u8; 32]) -> Self {
        U256::from_be_bytes(bytes).cast_signed()
    }
}

impl U256 {
    /// Reinterprets the bits as an [`I256`].
    ///
    /// No arithmetic: `U256::MAX.cast_signed()` is `-1`. Named after
    /// `u128::cast_signed`, which does the same thing at a narrower width.
    #[inline]
    pub const fn cast_signed(self) -> I256 {
        I256::from_limbs(self.limbs)
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

// As in the unsigned half, the bodies call inherent-only names so that their
// termination does not hang on the inherent `from` shadowing the trait.
macro_rules! impl_from_signed {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for I256 {
                #[inline]
                fn from(n: $t) -> Self {
                    Self::from_i32(n as i32)
                }
            }
        )*
    };
}

impl_from_signed!(i8, i16, i32);

impl From<i64> for I256 {
    #[inline]
    fn from(n: i64) -> Self {
        Self::from_i64(n)
    }
}

impl From<i128> for I256 {
    #[inline]
    fn from(n: i128) -> Self {
        Self::from_i128(n)
    }
}

// Every unsigned primitive fits: `u128::MAX` is well below `2^255 - 1`.
macro_rules! impl_from_unsigned {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for I256 {
                #[inline]
                fn from(n: $t) -> Self {
                    U256::from_u128(n as u128).cast_signed()
                }
            }
        )*
    };
}

impl_from_unsigned!(u8, u16, u32, u64, u128);

impl From<bool> for I256 {
    #[inline]
    fn from(b: bool) -> Self {
        if b { Self::ONE } else { Self::ZERO }
    }
}

impl TryFrom<U256> for I256 {
    type Error = TryFromIntError;

    /// Fails for values above `2^255 - 1`, which would land on a negative
    /// `I256`. Use [`U256::cast_signed`] to reinterpret those bits instead.
    #[inline]
    fn try_from(n: U256) -> Result<Self, Self::Error> {
        let v = n.cast_signed();
        if v.is_negative() {
            Err(TryFromIntError(()))
        } else {
            Ok(v)
        }
    }
}

impl TryFrom<I256> for U256 {
    type Error = TryFromIntError;

    /// Fails for negative values. Use [`I256::cast_unsigned`] to reinterpret
    /// those bits instead.
    #[inline]
    fn try_from(n: I256) -> Result<Self, Self::Error> {
        if n.is_negative() {
            Err(TryFromIntError(()))
        } else {
            Ok(n.cast_unsigned())
        }
    }
}

macro_rules! impl_try_into_signed {
    ($($signed:ty => $unsigned:ty),* $(,)?) => {
        $(
            impl TryFrom<I256> for $signed {
                type Error = TryFromIntError;

                #[inline]
                fn try_from(n: I256) -> Result<Self, Self::Error> {
                    let mag = <$unsigned>::try_from(n.unsigned_abs())
                        .map_err(|_| TryFromIntError(()))?;
                    if n.is_negative() {
                        // The negative range reaches one further, so the
                        // bound is on the magnitude, not on the positive
                        // value it would cast to.
                        if mag > (1 as $unsigned) << (<$signed>::BITS - 1) {
                            return Err(TryFromIntError(()));
                        }
                        // `mag == 2^(B-1)` casts to `MIN`, whose negation is
                        // itself -- exactly the answer wanted.
                        Ok((mag as $signed).wrapping_neg())
                    } else {
                        <$signed>::try_from(mag)
                            .map_err(|_| TryFromIntError(()))
                    }
                }
            }
        )*
    };
}

impl_try_into_signed!(
    i8 => u8,
    i16 => u16,
    i32 => u32,
    i64 => u64,
    i128 => u128,
);

macro_rules! impl_try_into_unsigned {
    ($($t:ty),* $(,)?) => {
        $(
            impl TryFrom<I256> for $t {
                type Error = TryFromIntError;

                #[inline]
                fn try_from(n: I256) -> Result<Self, Self::Error> {
                    if n.is_negative() {
                        return Err(TryFromIntError(()));
                    }
                    <$t>::try_from(n.cast_unsigned())
                        .map_err(|_| TryFromIntError(()))
                }
            }
        )*
    };
}

impl_try_into_unsigned!(u8, u16, u32, u64, u128);

impl PartialOrd for I256 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for I256 {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.const_cmp(*other)
    }
}
