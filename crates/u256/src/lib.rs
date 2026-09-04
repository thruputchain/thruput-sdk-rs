//! 256-bit integers stored as `[u32; 8]`, native to a 32-bit register file,
//! with the math utilities an Ethereum port reaches for.
//!
//! [`U256`] is the unsigned type and [`I256`] its two's complement signed
//! counterpart. Both carry the utility surface an Ethereum port expects:
//! OpenZeppelin's `Math.sol` on the unsigned side ([`U256::mul_div`] on an
//! exact 512-bit intermediate, [`U256::mul_shr`], modular arithmetic,
//! [`U256::integer_sqrt`] and the `log2`/`log10`/`log256` family with a
//! [`Rounding`] direction) and `SignedMath.sol` on the signed one
//! ([`I256::average`], [`I256::min`], [`I256::max`],
//! [`I256::unsigned_abs`]).
//!
//! Parity's `uint` stores four `u64` limbs and multiplies through `u128`
//! intermediates, which on riscv32 is a `compiler_builtins` call rather than an
//! instruction. Measured on the swap suite: its `Div`/`Rem` lowered to 11,300
//! bytes of rv32 text and its `integer_sqrt` to 12,732 -- together over a third
//! of the 64 KiB artifact code cap.
//!
//! Both types behave like the primitives on overflow: the operators wrap when
//! `overflow-checks` is off (as every contract profile sets it) and panic when
//! it is on. Use [`U256::checked_add`] and friends where it matters.
//!
//! [`I256`] costs far less than a second implementation. Addition,
//! subtraction, multiplication and the bitwise operations produce the same
//! bits on both types, so only comparison, division, the arithmetic right
//! shift and formatting are genuinely separate code -- see `signed`.
//!
//! Every `Display`, `FromStr`, `LowerHex` and `UpperHex` impl in this crate --
//! on the integers and on their error types alike -- sits behind the `fmt`
//! feature, so a guest build carries neither the decimal-division loop nor the
//! string literals those impls need. `sdk` enables `fmt` only under
//! `cfg(not(target_arch = "riscv32"))`, so a contract does not build them at
//! all rather than building them and relying on the linker to notice they are
//! dead: a `Display` bound on [`U256`] does not resolve for
//! `riscv32im-unknown-none-elf`. Cargo features are additive, so a dependency
//! that names this crate with `fmt` itself would still turn it back on --
//! nothing here can prevent that. `Debug` is the deliberate exception -- see
//! `fmt`.
//!
//! The keying is exact under the `nightly` feature (guest builds get it
//! through `sdk`), which reads rustc's real `overflow-checks` flag. Without
//! it — on a stable compiler, where that cfg does not exist — overflow
//! checks are always on: the operators panic on overflow in every profile.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// Keys the operators off the same flag rustc uses for the primitives. Behind
// a cargo feature so stable consumers (the CLI's spec-tool build) can compile
// this crate: without it the operators check overflow unconditionally --
// stable consumers are host tools, never gas-metered contracts, so the guard
// is cheap and never silently wraps. Guest builds always run nightly and get
// `nightly` through `sdk`'s target-gated dependency, keeping the exact keying.
#![cfg_attr(feature = "nightly", feature(cfg_overflow_checks))]

mod arith;
mod div;
mod fmt;
mod integration;
mod math;
mod signed;
mod signed_arith;
mod signed_fmt;
mod signed_math;
mod sqrt;

pub use div::{checked_mul_div_u128, mul_div_u128};
pub use math::Rounding;
pub use signed::I256;

#[cfg(feature = "fmt")]
pub use fmt::ParseU256Error;
#[cfg(feature = "fmt")]
pub use signed_fmt::ParseI256Error;

/// The number of 32-bit limbs in a [`U256`].
pub(crate) const LIMBS: usize = 8;

/// A 256-bit unsigned integer.
///
/// Limbs are least-significant-first in memory; the wire encoding is
/// most-significant-first (see `integration`), matching xdrs' `u128`.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U256 {
    pub(crate) limbs: [u32; LIMBS],
}

impl U256 {
    /// The additive identity.
    pub const ZERO: Self = Self::from_limbs([0; LIMBS]);

    /// The multiplicative identity.
    pub const ONE: Self = Self::from_limbs([1, 0, 0, 0, 0, 0, 0, 0]);

    /// The largest representable value.
    pub const MAX: Self = Self::from_limbs([u32::MAX; LIMBS]);

    /// The size of this integer in bits.
    pub const BITS: u32 = 256;

    /// Constructs a value from limbs, least-significant-first.
    #[inline]
    pub const fn from_limbs(limbs: [u32; LIMBS]) -> Self {
        Self { limbs }
    }

    /// Constructs a value from a `u128`.
    ///
    /// Inherent, and deliberately so. `From` is implemented for every unsigned
    /// width, which leaves `U256::from(1000)` ambiguous -- an unsuffixed
    /// literal matches all of them -- and an inherent function shadows the
    /// trait for the path form, pinning the literal to `u128`. It is also
    /// usable in const position, which `From::from` is not.
    ///
    /// The trait impls are untouched, so `.into()` and generic `T: Into<U256>`
    /// code still convert from any width. What this shadowing costs is
    /// `U256::from(x)` for an `x` narrower than `u128`; use `x.into()` or
    /// [`from_u32`](Self::from_u32) / [`from_u64`](Self::from_u64) there.
    #[inline]
    pub const fn from(n: u128) -> Self {
        Self::from_u128(n)
    }

    /// Constructs a value from a `u128`, under a name no trait shadows.
    ///
    /// [`from`](Self::from) is this function behind the literal-pinning
    /// shadow, and the `From<u128>` impl calls it too, so neither body
    /// depends on the shadowing to terminate.
    #[inline]
    pub const fn from_u128(n: u128) -> Self {
        Self::from_limbs([
            n as u32,
            (n >> 32) as u32,
            (n >> 64) as u32,
            (n >> 96) as u32,
            0,
            0,
            0,
            0,
        ])
    }

    /// Constructs a value from a `u32`, without widening at the call site.
    #[inline]
    pub const fn from_u32(n: u32) -> Self {
        Self::from_limbs([n, 0, 0, 0, 0, 0, 0, 0])
    }

    /// Constructs a value from a `u64`, without widening at the call site.
    #[inline]
    pub const fn from_u64(n: u64) -> Self {
        Self::from_limbs([n as u32, (n >> 32) as u32, 0, 0, 0, 0, 0, 0])
    }

    /// Returns the limbs, least-significant-first.
    #[inline]
    pub const fn to_limbs(self) -> [u32; LIMBS] {
        self.limbs
    }

    /// Returns `true` if this is zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        let mut i = 0;
        while i < LIMBS {
            if self.limbs[i] != 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    /// The number of significant bits: `0` for zero, otherwise one more than
    /// the index of the highest set bit.
    #[inline]
    pub const fn bit_len(self) -> u32 {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            if self.limbs[i] != 0 {
                return (i as u32) * 32 + (32 - self.limbs[i].leading_zeros());
            }
        }
        0
    }

    /// The number of leading zero bits.
    #[inline]
    pub const fn leading_zeros(self) -> u32 {
        Self::BITS - self.bit_len()
    }

    /// The number of significant limbs: `0` for zero, otherwise one more than
    /// the index of the highest nonzero limb.
    #[inline]
    pub(crate) const fn limb_len(self) -> usize {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            if self.limbs[i] != 0 {
                return i + 1;
            }
        }
        0
    }

    /// Returns the bit at `i`, counting from the least significant.
    ///
    /// Bits at or above [`BITS`](Self::BITS) are zero, rather than a panic:
    /// callers index by a computed position (an exponent bit, a mask width),
    /// and the out-of-range answer is unambiguous.
    #[inline]
    pub const fn bit(self, i: u32) -> bool {
        if i >= Self::BITS {
            return false;
        }
        (self.limbs[(i / 32) as usize] >> (i % 32)) & 1 == 1
    }

    /// Comparison usable from a `const fn`.
    ///
    /// The [`Ord`] impl is the one callers want; it just is not `const`, and
    /// the arithmetic in this crate is. Same ordering, same answer.
    #[inline]
    pub const fn const_cmp(self, other: Self) -> core::cmp::Ordering {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            if self.limbs[i] < other.limbs[i] {
                return core::cmp::Ordering::Less;
            }
            if self.limbs[i] > other.limbs[i] {
                return core::cmp::Ordering::Greater;
            }
        }
        core::cmp::Ordering::Equal
    }

    /// `self == other`, from a `const fn`.
    #[inline]
    pub(crate) const fn const_eq(self, other: Self) -> bool {
        matches!(self.const_cmp(other), core::cmp::Ordering::Equal)
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

    /// `self >= other`, from a `const fn`.
    #[inline]
    pub(crate) const fn const_ge(self, other: Self) -> bool {
        !self.const_lt(other)
    }

    /// Returns the big-endian byte representation.
    ///
    /// Outlined on rv32 -- every contract that stores a `U256` pays for this
    /// one. See the inlining note in `arith`.
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    pub const fn to_be_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut i = 0;
        while i < LIMBS {
            // Limb `i` is the `i`th least significant, so it occupies the
            // `i`th group of four bytes counting back from the end.
            let bytes = self.limbs[LIMBS - 1 - i].to_be_bytes();
            out[i * 4] = bytes[0];
            out[i * 4 + 1] = bytes[1];
            out[i * 4 + 2] = bytes[2];
            out[i * 4 + 3] = bytes[3];
            i += 1;
        }
        out
    }

    /// Constructs a value from its big-endian byte representation.
    ///
    /// Outlined on rv32; see the inlining note in `arith`.
    #[cfg_attr(target_arch = "riscv32", inline(never))]
    pub const fn from_be_bytes(bytes: [u8; 32]) -> Self {
        let mut limbs = [0u32; LIMBS];
        let mut i = 0;
        while i < LIMBS {
            limbs[LIMBS - 1 - i] = u32::from_be_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ]);
            i += 1;
        }
        Self::from_limbs(limbs)
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

// The bodies below all call inherent-only names (`from_u32`, `from_u64`,
// `from_u128`), never `Self::from`. That spelling would compile today -- the
// inherent `from` shadows the trait in path form -- but its termination would
// hang on the shadowing: rename the inherent fn and `Self::from(n)` inside
// `impl From<u128>` re-resolves to the trait impl and recurses, with only a
// warn-level `unconditional_recursion` lint standing in the way.
macro_rules! impl_from_small {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for U256 {
                #[inline]
                fn from(n: $t) -> Self {
                    Self::from_u32(n as u32)
                }
            }
        )*
    };
}

impl_from_small!(u8, u16, u32);

impl From<u64> for U256 {
    #[inline]
    fn from(n: u64) -> Self {
        Self::from_u64(n)
    }
}

impl From<u128> for U256 {
    #[inline]
    fn from(n: u128) -> Self {
        Self::from_u128(n)
    }
}

impl From<bool> for U256 {
    #[inline]
    fn from(b: bool) -> Self {
        if b { Self::ONE } else { Self::ZERO }
    }
}

/// Returned when an integer conversion in this crate does not fit.
///
/// One type covers every direction -- [`U256`] or [`I256`] down to a narrower
/// primitive, and between the two 256-bit types -- and every reason, whether
/// the value is out of the target's range or, converting to an unsigned type,
/// negative. That is what `core::num::TryFromIntError` does:
/// `u8::try_from(300u32)` and `u8::try_from(-1i32)` both yield it, with no
/// split by the source's signedness and no payload to inspect. Splitting it
/// here would add a type without adding information.
///
/// It is a distinct type from `core::num::TryFromIntError` only because that
/// one cannot be constructed outside `core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromIntError(pub(crate) ());

/// Gated on `fmt` like every other `Display` in this crate: the message is a
/// string literal, and a guest build that never formats the error should not
/// carry it. `std` implies `fmt`, so the `Error` impl below always has its
/// `Display` bound.
#[cfg(feature = "fmt")]
impl core::fmt::Display for TryFromIntError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("value does not fit in the target type")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TryFromIntError {}

macro_rules! impl_try_into {
    ($($t:ty => $limbs:expr),* $(,)?) => {
        $(
            impl TryFrom<U256> for $t {
                type Error = TryFromIntError;

                #[inline]
                fn try_from(n: U256) -> Result<Self, Self::Error> {
                    if n.limb_len() > $limbs {
                        return Err(TryFromIntError(()));
                    }
                    let mut out: $t = 0;
                    let mut i = $limbs;
                    while i > 0 {
                        i -= 1;
                        out = (out << 16 << 16) | (n.limbs[i] as $t);
                    }
                    Ok(out)
                }
            }
        )*
    };
}

// `<< 16 << 16` rather than `<< 32`: for `u32` a 32-bit shift is UB-adjacent
// (it panics/wraps depending on profile), and this form stays correct for
// every width without a special case.
impl_try_into!(u32 => 1, u64 => 2, u128 => 4);

/// Targets narrower than a limb, where the limb count is not the bound.
///
/// `impl_try_into!` keys its check on `limb_len`, which cannot express "fits
/// in eight bits", and its `<< 16 << 16` accumulator would overflow the target
/// outright. Narrowing to `u32` first and letting that type's own conversion
/// finish the job is both correct and smaller than a bespoke bound.
///
/// These exist for parity with OpenZeppelin's `SafeCast`, whose `toUint8`
/// through `toUint128` a port will reach for by name.
macro_rules! impl_try_into_narrow {
    ($($t:ty),* $(,)?) => {
        $(
            impl TryFrom<U256> for $t {
                type Error = TryFromIntError;

                #[inline]
                fn try_from(n: U256) -> Result<Self, Self::Error> {
                    let narrowed = u32::try_from(n)?;
                    <$t>::try_from(narrowed)
                        .map_err(|_| TryFromIntError(()))
                }
            }
        )*
    };
}

impl_try_into_narrow!(u8, u16);

impl PartialOrd for U256 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for U256 {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            if self.limbs[i] != other.limbs[i] {
                return self.limbs[i].cmp(&other.limbs[i]);
            }
        }
        core::cmp::Ordering::Equal
    }
}
