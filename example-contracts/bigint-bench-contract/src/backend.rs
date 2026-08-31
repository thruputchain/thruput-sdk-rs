//! The operation set every benchmarked implementation must provide.
//!
//! One trait, one impl per implementation under test. The kernels in
//! [`crate::ops`] are generic over this trait, so a build instantiates it
//! exactly once and LTO sees the same shape it would see if the contract had
//! called the implementation directly.
//!
//! ## Mapping rule
//!
//! Each method maps to the API a contract author would actually reach for in
//! that implementation, at that implementation's native limb width -- not to
//! the closest literal spelling. `wrapping_mul_small` is the clear case:
//! `sdk::U256` takes a `u32` scalar (`overflowing_mul_u32`) and `uint` takes a
//! `u64` one (`overflowing_mul_u64`), because that is each library's
//! single-limb multiply. Comparing U256's u32 path against a full 256x256
//! `uint` multiply would measure the wrong thing. The mathematical results
//! still agree for any scalar that fits in 32 bits, which is what
//! `tests/differential.rs` asserts.

/// A 256-bit unsigned integer implementation under measurement.
pub trait Backend: Copy + PartialEq + core::fmt::Debug {
    /// Short name used in the report (`u256`, `uint`).
    const NAME: &'static str;

    /// Width in bits. Shift distances are reduced modulo this.
    const BITS: u32;

    /// The multiplicative identity. Used to force a divisor nonzero.
    fn one() -> Self;

    fn from_u128(v: u128) -> Self;

    /// Reassembles a value from `(high, low)` 128-bit halves.
    fn from_hi_lo(hi: u128, lo: u128) -> Self;

    fn from_be_bytes(bytes: [u8; 32]) -> Self;
    fn to_be_bytes(self) -> [u8; 32];

    fn low_u32(self) -> u32;
    fn low_u128(self) -> u128;

    /// Folds every limb into a `u64`.
    ///
    /// The contract returns this so the kernel's result stays live. It reads
    /// the implementation's own limbs rather than going through
    /// [`Backend::to_be_bytes`] deliberately: routing it through the byte
    /// conversion would put that conversion in *every* build, and the
    /// `op-bytes` variant would then measure a marginal cost of roughly zero.
    fn digest(self) -> u64;

    fn wrapping_add(self, rhs: Self) -> Self;
    fn wrapping_sub(self, rhs: Self) -> Self;
    fn wrapping_mul(self, rhs: Self) -> Self;

    /// Wrapping multiply by a single-limb scalar. See the mapping rule above.
    fn wrapping_mul_small(self, rhs: u32) -> Self;

    fn div(self, rhs: Self) -> Self;
    fn rem(self, rhs: Self) -> Self;

    /// Divide by a single-limb divisor.
    fn div_small(self, rhs: u32) -> Self;

    fn shl(self, n: u32) -> Self;
    fn shr(self, n: u32) -> Self;

    fn bitand(self, rhs: Self) -> Self;
    fn bitor(self, rhs: Self) -> Self;
    fn bitxor(self, rhs: Self) -> Self;
    fn not(self) -> Self;

    fn lt(self, rhs: Self) -> bool;

    /// Number of significant bits: `0` for zero.
    fn bit_len(self) -> u32;

    /// The largest `r` with `r * r <= self`.
    fn integer_sqrt(self) -> Self;

    /// The exact 256-bit product of two 128-bit values.
    fn widening_mul_u128(a: u128, b: u128) -> Self;

    /// `a * b / d`, computed exactly through a 256-bit intermediate.
    ///
    /// Callers must pass a `d` for which the quotient fits in a `u128`;
    /// [`crate::ops::mul_div_u128`] derives one that always does.
    fn mul_div_u128(a: u128, b: u128, d: u128) -> u128;

    /// `a * b / d` over full-width operands, computed exactly.
    ///
    /// The 512-bit shape: unlike [`mul_div_u128`](Backend::mul_div_u128) the
    /// operands fill the type, so the intermediate product genuinely needs
    /// twice its width and neither backend can hold it in one value.
    ///
    /// Callers must pass a `d` for which the quotient fits;
    /// [`crate::ops::mul_div`] derives one that always does.
    fn mul_div(a: Self, b: Self, d: Self) -> Self;

    /// Signed multiply with saturation, over the type's bits read as two's
    /// complement.
    ///
    /// Saturating rather than wrapping on purpose: a *wrapping* signed
    /// multiply is bit-identical to the unsigned one, so it would measure
    /// [`wrapping_mul`](Backend::wrapping_mul) over again. Saturating keeps
    /// the overflow flag live, which is where the signed work actually is.
    ///
    /// `uint` has no signed type at all, so its implementation is the
    /// sign-magnitude wrapper a contract author on that backend would have to
    /// write -- the same reasoning as the 512-bit type behind `mul_div`.
    fn signed_saturating_mul(self, rhs: Self) -> Self;

    /// Signed truncating division, over the type's bits read as two's
    /// complement. Quotient truncates toward zero.
    fn signed_div(self, rhs: Self) -> Self;
}

/// `sdk::U256`: eight 32-bit limbs, native to a 32-bit register file.
pub mod u256 {
    use super::Backend;
    use sdk::U256;

    /// Newtype so this crate owns the trait impl.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct GroundhogU256(pub U256);

    impl Backend for GroundhogU256 {
        const NAME: &'static str = "u256";
        const BITS: u32 = U256::BITS;

        #[inline]
        fn one() -> Self {
            Self(U256::ONE)
        }

        #[inline]
        fn from_u128(v: u128) -> Self {
            Self(U256::from(v))
        }

        #[inline]
        fn from_hi_lo(hi: u128, lo: u128) -> Self {
            let mut bytes = [0u8; 32];
            bytes[..16].copy_from_slice(&hi.to_be_bytes());
            bytes[16..].copy_from_slice(&lo.to_be_bytes());
            Self(U256::from_be_bytes(bytes))
        }

        #[inline]
        fn from_be_bytes(bytes: [u8; 32]) -> Self {
            Self(U256::from_be_bytes(bytes))
        }

        #[inline]
        fn to_be_bytes(self) -> [u8; 32] {
            self.0.to_be_bytes()
        }

        #[inline]
        fn low_u32(self) -> u32 {
            self.0.to_limbs()[0]
        }

        #[inline]
        fn low_u128(self) -> u128 {
            let l = self.0.to_limbs();
            (l[0] as u128)
                | ((l[1] as u128) << 32)
                | ((l[2] as u128) << 64)
                | ((l[3] as u128) << 96)
        }

        #[inline]
        fn digest(self) -> u64 {
            let mut acc = 0u64;
            for limb in self.0.to_limbs() {
                acc = acc.rotate_left(7) ^ (limb as u64);
            }
            acc
        }

        #[inline]
        fn wrapping_add(self, rhs: Self) -> Self {
            Self(self.0.wrapping_add(rhs.0))
        }

        #[inline]
        fn wrapping_sub(self, rhs: Self) -> Self {
            Self(self.0.wrapping_sub(rhs.0))
        }

        #[inline]
        fn wrapping_mul(self, rhs: Self) -> Self {
            Self(self.0.wrapping_mul(rhs.0))
        }

        #[inline]
        fn wrapping_mul_small(self, rhs: u32) -> Self {
            Self(self.0.overflowing_mul_u32(rhs).0)
        }

        #[inline]
        fn div(self, rhs: Self) -> Self {
            Self(self.0.div_rem(rhs.0).0)
        }

        #[inline]
        fn rem(self, rhs: Self) -> Self {
            Self(self.0.div_rem(rhs.0).1)
        }

        #[inline]
        fn div_small(self, rhs: u32) -> Self {
            Self(self.0.div_rem_small(rhs).0)
        }

        #[inline]
        fn shl(self, n: u32) -> Self {
            Self(self.0.wrapping_shl(n))
        }

        #[inline]
        fn shr(self, n: u32) -> Self {
            Self(self.0.wrapping_shr(n))
        }

        #[inline]
        fn bitand(self, rhs: Self) -> Self {
            Self(self.0 & rhs.0)
        }

        #[inline]
        fn bitor(self, rhs: Self) -> Self {
            Self(self.0 | rhs.0)
        }

        #[inline]
        fn bitxor(self, rhs: Self) -> Self {
            Self(self.0 ^ rhs.0)
        }

        #[inline]
        fn not(self) -> Self {
            Self(!self.0)
        }

        #[inline]
        fn lt(self, rhs: Self) -> bool {
            self.0 < rhs.0
        }

        #[inline]
        fn bit_len(self) -> u32 {
            self.0.bit_len()
        }

        #[inline]
        fn integer_sqrt(self) -> Self {
            Self(self.0.integer_sqrt())
        }

        #[inline]
        fn widening_mul_u128(a: u128, b: u128) -> Self {
            Self(U256::widening_mul(a, b))
        }

        #[inline]
        fn mul_div_u128(a: u128, b: u128, d: u128) -> u128 {
            sdk::mul_div_u128(a, b, d)
        }

        #[inline]
        fn mul_div(a: Self, b: Self, d: Self) -> Self {
            Self(U256::mul_div(a.0, b.0, d.0))
        }

        #[inline]
        fn signed_saturating_mul(self, rhs: Self) -> Self {
            Self(
                self.0
                    .cast_signed()
                    .saturating_mul(rhs.0.cast_signed())
                    .cast_unsigned(),
            )
        }

        #[inline]
        fn signed_div(self, rhs: Self) -> Self {
            // `wrapping_div` rather than `/`: `MIN / -1` is a legitimate input
            // here and the operator aborts on it, which would end the sweep
            // rather than measure it.
            Self(
                self.0
                    .cast_signed()
                    .wrapping_div(rhs.0.cast_signed())
                    .cast_unsigned(),
            )
        }
    }
}

/// Parity's `uint`: four 64-bit limbs with `u128` intermediates.
///
/// The implementation `groundhog-u256` replaces, kept here as the comparison
/// baseline. Behind `backend-uint` so a `u256` build never compiles it.
#[cfg(feature = "backend-uint")]
pub mod parity {
    // `construct_uint!` open-codes a `div_ceil`; same allow the swap example
    // needs for the same macro.
    #![allow(clippy::manual_div_ceil)]

    use super::Backend;

    uint::construct_uint! {
        pub struct PU256(4);
    }

    uint::construct_uint! {
        /// The 512-bit intermediate the full-precision `mul_div` needs.
        ///
        /// `construct_uint!` generates no widening between two types it
        /// constructed separately -- `full_mul` exists only for the pairings
        /// inside `uint` itself -- so a contract author on this backend has to
        /// declare the wide type and move limbs across by hand. That is
        /// exactly what [`widen`] does below, which is what makes this a fair
        /// baseline rather than a strawman: it is the code that author would
        /// write.
        pub struct PU512(8);
    }

    /// Zero-extends a `PU256` into the wide type, limb for limb.
    #[inline]
    fn widen(v: PU256) -> PU512 {
        PU512([v.0[0], v.0[1], v.0[2], v.0[3], 0, 0, 0, 0])
    }

    /// `2^255`: `MIN`'s magnitude, and its bit pattern.
    #[inline]
    fn min_magnitude() -> PU256 {
        PU256::one() << 255
    }

    /// The sign bit of a value read as two's complement.
    #[inline]
    fn is_negative(v: PU256) -> bool {
        v.bit(255)
    }

    /// Two's complement negation: `!v + 1`.
    #[inline]
    fn negate(v: PU256) -> PU256 {
        (!v).overflowing_add(PU256::one()).0
    }

    /// Magnitude of a value read as two's complement. Total: `MIN` maps to
    /// `2^255`, which the unsigned type holds.
    #[inline]
    fn abs(v: PU256) -> PU256 {
        if is_negative(v) { negate(v) } else { v }
    }

    /// Truncates a `PU512` to its low four limbs.
    ///
    /// The callers below establish that the discarded half is zero.
    #[inline]
    fn narrow(v: PU512) -> PU256 {
        debug_assert!(
            v.0[4] == 0 && v.0[5] == 0 && v.0[6] == 0 && v.0[7] == 0
        );
        PU256([v.0[0], v.0[1], v.0[2], v.0[3]])
    }

    impl Backend for PU256 {
        const NAME: &'static str = "uint";
        const BITS: u32 = 256;

        #[inline]
        fn one() -> Self {
            Self::one()
        }

        #[inline]
        fn from_u128(v: u128) -> Self {
            Self::from(v)
        }

        #[inline]
        fn from_hi_lo(hi: u128, lo: u128) -> Self {
            let mut bytes = [0u8; 32];
            bytes[..16].copy_from_slice(&hi.to_be_bytes());
            bytes[16..].copy_from_slice(&lo.to_be_bytes());
            Self::from_big_endian(&bytes)
        }

        #[inline]
        fn from_be_bytes(bytes: [u8; 32]) -> Self {
            Self::from_big_endian(&bytes)
        }

        #[inline]
        fn to_be_bytes(self) -> [u8; 32] {
            self.to_big_endian()
        }

        #[inline]
        fn low_u32(self) -> u32 {
            PU256::low_u32(&self)
        }

        #[inline]
        fn low_u128(self) -> u128 {
            PU256::low_u128(&self)
        }

        #[inline]
        fn digest(self) -> u64 {
            let mut acc = 0u64;
            // Two rotations per 64-bit limb so the fold covers the same bit
            // positions as the eight-limb version in `u256`.
            for limb in AsRef::<[u64]>::as_ref(&self) {
                acc = acc.rotate_left(7) ^ (*limb & 0xffff_ffff);
                acc = acc.rotate_left(7) ^ (*limb >> 32);
            }
            acc
        }

        #[inline]
        fn wrapping_add(self, rhs: Self) -> Self {
            self.overflowing_add(rhs).0
        }

        #[inline]
        fn wrapping_sub(self, rhs: Self) -> Self {
            self.overflowing_sub(rhs).0
        }

        #[inline]
        fn wrapping_mul(self, rhs: Self) -> Self {
            self.overflowing_mul(rhs).0
        }

        #[inline]
        fn wrapping_mul_small(self, rhs: u32) -> Self {
            // u64 scalar: `uint`'s single-limb multiply. See the mapping rule.
            self.overflowing_mul_u64(rhs as u64).0
        }

        #[inline]
        fn div(self, rhs: Self) -> Self {
            self / rhs
        }

        #[inline]
        fn rem(self, rhs: Self) -> Self {
            self % rhs
        }

        #[inline]
        fn div_small(self, rhs: u32) -> Self {
            // `div_mod` fast-paths to its single-limb divide when the divisor
            // fits in one 64-bit word, so this is `uint`'s small-divisor path
            // reached the way a contract author reaches it.
            self / Self::from(rhs)
        }

        #[inline]
        fn shl(self, n: u32) -> Self {
            self << n
        }

        #[inline]
        fn shr(self, n: u32) -> Self {
            self >> n
        }

        #[inline]
        fn bitand(self, rhs: Self) -> Self {
            self & rhs
        }

        #[inline]
        fn bitor(self, rhs: Self) -> Self {
            self | rhs
        }

        #[inline]
        fn bitxor(self, rhs: Self) -> Self {
            self ^ rhs
        }

        #[inline]
        fn not(self) -> Self {
            !self
        }

        #[inline]
        fn lt(self, rhs: Self) -> bool {
            self < rhs
        }

        #[inline]
        fn bit_len(self) -> u32 {
            self.bits() as u32
        }

        #[inline]
        fn integer_sqrt(self) -> Self {
            PU256::integer_sqrt(&self)
        }

        #[inline]
        fn widening_mul_u128(a: u128, b: u128) -> Self {
            // No widening entry point: the product of two u128s cannot
            // overflow 256 bits, so the general multiply is the equivalent.
            Self::from(a).overflowing_mul(Self::from(b)).0
        }

        #[inline]
        fn mul_div_u128(a: u128, b: u128, d: u128) -> u128 {
            let q =
                Self::from(a).overflowing_mul(Self::from(b)).0 / Self::from(d);
            PU256::low_u128(&q)
        }

        #[inline]
        fn mul_div(a: Self, b: Self, d: Self) -> Self {
            // Widen first: the product of two full-width values does not fit
            // the narrow type, which is the whole reason this op exists. It
            // cannot overflow `PU512`, and `d` is nonzero by the caller's
            // construction, so neither operator can panic.
            narrow(widen(a) * widen(b) / widen(d))
        }

        #[inline]
        fn signed_saturating_mul(self, rhs: Self) -> Self {
            let (mag, wide) = abs(self).overflowing_mul(abs(rhs));
            let negative = is_negative(self) != is_negative(rhs);
            let min = min_magnitude();
            // The negative range reaches one further than the positive one.
            let limit = if negative { min } else { min - PU256::one() };
            if wide || mag > limit {
                // Saturate to MIN's bits or MAX's, by the product's sign.
                return if negative { min } else { min - PU256::one() };
            }
            if negative { negate(mag) } else { mag }
        }

        #[inline]
        fn signed_div(self, rhs: Self) -> Self {
            let q = abs(self) / abs(rhs);
            if is_negative(self) != is_negative(rhs) {
                negate(q)
            } else {
                q
            }
        }
    }
}
