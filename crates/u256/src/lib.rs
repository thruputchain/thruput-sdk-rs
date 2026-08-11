//! A 256-bit unsigned integer whose representation is native to a 32-bit
//! register file.
//!
//! # Why this exists
//!
//! Parity's `uint` crate (`uint::construct_uint!`) is correct but badly
//! matched to our guest targets. It stores four `u64` limbs and reaches for
//! `u128` intermediates in every widening multiply. On riscv32 a
//! `u64 * u64 -> u128` is not two instructions; it is a `compiler_builtins`
//! call with the `u128` passed indirectly through memory. wasm32 has native
//! `i64` but no native `i128`, so it pays the same tax.
//!
//! The size cost was measured on the swap suite before this crate existed:
//! `uint`'s Knuth-D `Div`/`Rem` lowered to 11,300 bytes of rv32 text, and its
//! Newton-iteration `integer_sqrt` to 12,732 bytes -- together over a third of
//! the artifact format's 64 KiB code cap, for a contract that divides three
//! times.
//!
//! Storing `[u32; 8]` puts one limb per register and lets a widening multiply
//! be `mul`/`mulhu` pairs with no libcall. On wasm32, `u32 * u32 -> u64`
//! lowers to `i64.extend_u` plus `i64.mul`, both native, so the same
//! representation wins there too.
//!
//! # Overflow behaviour
//!
//! [`U256`] behaves exactly like `u128`: the arithmetic operators wrap when
//! overflow checks are off and panic when they are on. Contract profiles set
//! `overflow-checks = false`, so a release contract build wraps -- matching the
//! `u128` sitting next to it in the same expression. Host test builds panic.
//!
//! Use [`U256::checked_add`] and friends where the overflow matters.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// Lets the operators key off the same flag rustc uses for the primitive
// integers, so `U256` and `u128` overflow identically under every profile.
// If this crate ever needs to build without nightly features, swapping the
// `cfg(overflow_checks)` sites to `cfg(debug_assertions)` is faithful under
// every profile in this workspace -- all of them set the two flags together.
#![feature(cfg_overflow_checks)]

mod arith;
mod div;
mod fmt;
mod integration;
mod sqrt;

pub use div::{checked_mul_div_u128, mul_div_u128};

#[cfg(feature = "fmt")]
pub use fmt::ParseU256Error;

// Re-exported so the linker anchor and raw typedef fragment are reachable the
// same way a `#[contract_type]` type's are.
#[doc(hidden)]
pub use integration::{
    __GROUNDHOG_SPEC_FRAGMENT_U256, __groundhog_spec_anchor_fn_u256,
};

/// The number of 32-bit limbs in a [`U256`].
pub(crate) const LIMBS: usize = 8;

/// A 256-bit unsigned integer.
///
/// Limbs are stored least-significant-first. The wire encoding is
/// most-significant-first (see the `integration` module), matching the
/// `(high, low)` convention xdrs already uses for `u128`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U256(pub(crate) [u32; LIMBS]);

impl U256 {
    /// The additive identity.
    pub const ZERO: Self = Self([0; LIMBS]);

    /// The multiplicative identity.
    pub const ONE: Self = Self([1, 0, 0, 0, 0, 0, 0, 0]);

    /// The largest representable value.
    pub const MAX: Self = Self([u32::MAX; LIMBS]);

    /// The size of this integer in bits.
    pub const BITS: u32 = 256;

    /// Constructs a value from limbs, least-significant-first.
    #[inline]
    pub const fn from_limbs(limbs: [u32; LIMBS]) -> Self {
        Self(limbs)
    }

    /// Returns the limbs, least-significant-first.
    #[inline]
    pub const fn to_limbs(self) -> [u32; LIMBS] {
        self.0
    }

    /// Returns `true` if this is zero.
    #[inline]
    pub const fn is_zero(self) -> bool {
        let mut i = 0;
        while i < LIMBS {
            if self.0[i] != 0 {
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
            if self.0[i] != 0 {
                return (i as u32) * 32 + (32 - self.0[i].leading_zeros());
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
            if self.0[i] != 0 {
                return i + 1;
            }
        }
        0
    }

    /// Returns the big-endian byte representation.
    #[inline]
    pub const fn to_be_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut i = 0;
        while i < LIMBS {
            // Limb `i` is the `i`th least significant, so it occupies the
            // `i`th group of four bytes counting back from the end.
            let bytes = self.0[LIMBS - 1 - i].to_be_bytes();
            out[i * 4] = bytes[0];
            out[i * 4 + 1] = bytes[1];
            out[i * 4 + 2] = bytes[2];
            out[i * 4 + 3] = bytes[3];
            i += 1;
        }
        out
    }

    /// Constructs a value from its big-endian byte representation.
    #[inline]
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
        Self(limbs)
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

macro_rules! impl_from_small {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for U256 {
                #[inline]
                fn from(n: $t) -> Self {
                    let mut limbs = [0u32; LIMBS];
                    limbs[0] = n as u32;
                    Self(limbs)
                }
            }
        )*
    };
}

impl_from_small!(u8, u16, u32);

impl From<u64> for U256 {
    #[inline]
    fn from(n: u64) -> Self {
        let mut limbs = [0u32; LIMBS];
        limbs[0] = n as u32;
        limbs[1] = (n >> 32) as u32;
        Self(limbs)
    }
}

impl From<u128> for U256 {
    #[inline]
    fn from(n: u128) -> Self {
        let mut limbs = [0u32; LIMBS];
        limbs[0] = n as u32;
        limbs[1] = (n >> 32) as u32;
        limbs[2] = (n >> 64) as u32;
        limbs[3] = (n >> 96) as u32;
        Self(limbs)
    }
}

impl From<bool> for U256 {
    #[inline]
    fn from(b: bool) -> Self {
        if b { Self::ONE } else { Self::ZERO }
    }
}

/// Returned when a [`U256`] does not fit the requested narrower type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromU256Error(());

impl core::fmt::Display for TryFromU256Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("value does not fit in the target type")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TryFromU256Error {}

macro_rules! impl_try_into {
    ($($t:ty => $limbs:expr),* $(,)?) => {
        $(
            impl TryFrom<U256> for $t {
                type Error = TryFromU256Error;

                #[inline]
                fn try_from(n: U256) -> Result<Self, Self::Error> {
                    if n.limb_len() > $limbs {
                        return Err(TryFromU256Error(()));
                    }
                    let mut out: $t = 0;
                    let mut i = $limbs;
                    while i > 0 {
                        i -= 1;
                        out = (out << 16 << 16) | (n.0[i] as $t);
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
            if self.0[i] != other.0[i] {
                return self.0[i].cmp(&other.0[i]);
            }
        }
        core::cmp::Ordering::Equal
    }
}
