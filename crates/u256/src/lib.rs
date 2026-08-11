//! A 256-bit unsigned integer stored as `[u32; 8]`, native to a 32-bit
//! register file.
//!
//! Parity's `uint` stores four `u64` limbs and multiplies through `u128`
//! intermediates, which on riscv32 is a `compiler_builtins` call rather than an
//! instruction. Measured on the swap suite: its `Div`/`Rem` lowered to 11,300
//! bytes of rv32 text and its `integer_sqrt` to 12,732 -- together over a third
//! of the 64 KiB artifact code cap.
//!
//! [`U256`] behaves like `u128` on overflow: the operators wrap when
//! `overflow-checks` is off (as every contract profile sets it) and panic when
//! it is on. Use [`U256::checked_add`] and friends where it matters.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// Keys the operators off the same flag rustc uses for the primitives.
// `cfg(debug_assertions)` is a faithful stable substitute here -- every profile
// in this workspace moves the two together.
#![feature(cfg_overflow_checks)]

mod arith;
mod div;
mod fmt;
mod integration;
mod sqrt;

pub use div::{checked_mul_div_u128, mul_div_u128};

#[cfg(feature = "fmt")]
pub use fmt::ParseU256Error;

/// Path shim for `#[derive(ContractTypeSpec)]`. The derive would otherwise emit
/// `::sdk::…`, and `sdk` depends on this crate; both names it needs are just
/// `sdk` re-exporting a crate below it.
#[doc(hidden)]
pub mod __sdk_paths {
    pub use groundhog_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}

/// The number of 32-bit limbs in a [`U256`].
pub(crate) const LIMBS: usize = 8;

/// A 256-bit unsigned integer.
///
/// Limbs are least-significant-first in memory; the wire encoding is
/// most-significant-first (see `integration`), matching xdrs' `u128`.
///
/// Named field rather than a tuple because the spec derive would call an
/// unnamed field `_0`, and `limbs` is wire-visible.
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

    /// Returns the big-endian byte representation.
    #[inline]
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
        Self::from_limbs(limbs)
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
                    Self::from_limbs(limbs)
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
        Self::from_limbs(limbs)
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
        Self::from_limbs(limbs)
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
