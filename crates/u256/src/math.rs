//! Integer math utilities: OpenZeppelin's `Math.sol`, in Rust.
//!
//! Contract authors porting from Solidity reach for `Math.mulDiv`,
//! `Math.sqrt`, `Math.log10` and the rounding modes by name, so the names
//! here are the same modulo Rust casing, and each function is documented
//! against the Solidity one it mirrors.
//!
//! Three groups of `Math.sol`'s members have no function here, because Rust
//! already spells them:
//!
//! * `tryAdd`/`trySub`/`tryMul`/`tryDiv`/`tryMod` are
//!   [`checked_add`](U256::checked_add) and friends. Solidity returns
//!   `(bool, uint256)` because it has no `Option`.
//! * `ternary` is a branchless-select trick for the EVM's gas model. In Rust
//!   it is `if`.
//! * `clz` is [`leading_zeros`](U256::leading_zeros), which already answers
//!   256 for zero exactly as `Math.clz` does.
//!
//! One name differs: `Math.sqrt` is [`integer_sqrt`](U256::integer_sqrt), the
//! spelling this crate shipped with, plus
//! [`sqrt_rounding`](U256::sqrt_rounding) for the variant that takes a
//! direction.
//!
//! # 512-bit values
//!
//! There is no `U512` here, and none is needed for parity: Solidity has no type
//! wider than `uint256` either, so `Math.sol` returns its two 512-bit results
//! as a `(high, low)` pair of `uint256`s. Both have a counterpart --
//! `Math.mul512` is [`widening_mul_u256`](U256::widening_mul_u256) and
//! `Math.add512` is [`overflowing_add`](U256::overflowing_add), each ordered
//! `(low, high)` to match `u128::widening_mul` rather than Solidity.
//!
//! The only consumers of a 512-bit intermediate in `Math.sol` are `mulDiv` and
//! `mulShr`, and both are here whole, so the pair never has to be arithmetic
//! the caller does themselves.
//!
//! # Code size
//!
//! [`mul_div`](U256::mul_div) and the other full-precision entry points link
//! their own instantiation of Knuth D, sized to a 512-bit dividend, plus the
//! 512-bit multiply that the narrower [`U256::widening_mul`] does not need.
//! Measured by `scripts/bigint-bench.sh -o mul_div`, rv32: **3,992 bytes** of
//! `.text` and 4712 gas per call, against 2,572 and 972 for a plain `/`. The
//! same operation over Parity's `uint` -- which has no full-precision `mulDiv`,
//! so the baseline widens to a `construct_uint!(PU512(8))` by hand, as a
//! contract author there would -- costs 21,568 bytes and 22,625 gas: a third of
//! the 64 KiB artifact code cap, for one operation.
//!
//! A contract that uses both `/` and this family links Knuth D twice; `div`'s
//! `knuth_d` documents why that beats sharing one copy, with the numbers.
//! [`mod_exp`](U256::mod_exp) and [`inv_mod`](U256::inv_mod) are loops over
//! the multiply-divide pair -- reach for them knowingly.

use crate::div::{WIDE_LIMBS, div_rem_512, low_limbs};
use crate::{LIMBS, U256};

/// The rounding direction of an inexact result.
///
/// `Math.Rounding` in Solidity, with the same four cases and the same
/// discriminant order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Rounding {
    /// Toward negative infinity. For an unsigned value, the same as
    /// [`Trunc`](Self::Trunc).
    #[default]
    Floor,
    /// Toward positive infinity. For an unsigned value, the same as
    /// [`Expand`](Self::Expand).
    Ceil,
    /// Toward zero.
    Trunc,
    /// Away from zero.
    Expand,
}

impl Rounding {
    /// Whether this direction rounds an inexact *unsigned* result up.
    ///
    /// `Math.unsignedRoundsUp`. Both signed directions collapse for a value
    /// that cannot be negative: rounding toward positive infinity and rounding
    /// away from zero are the same instruction.
    #[inline]
    pub const fn unsigned_rounds_up(self) -> bool {
        matches!(self, Self::Ceil | Self::Expand)
    }

    /// Whether this direction rounds an inexact result of the given sign
    /// *away from zero*.
    ///
    /// The signed counterpart of [`unsigned_rounds_up`](Self::unsigned_rounds_up):
    /// [`Floor`](Self::Floor) grows a negative magnitude, [`Ceil`](Self::Ceil)
    /// a positive one.
    #[inline]
    pub const fn rounds_away_from_zero(self, negative: bool) -> bool {
        match self {
            Self::Floor => negative,
            Self::Ceil => !negative,
            Self::Trunc => false,
            Self::Expand => true,
        }
    }
}

impl U256 {
    // -----------------------------------------------------------------------
    // Comparison and averaging
    // -----------------------------------------------------------------------

    /// The smaller of two values. `Math.min`.
    ///
    /// Shadows [`Ord::min`] in method position, which resolves to the same
    /// answer; this one is a `const fn`.
    #[inline]
    pub const fn min(self, other: Self) -> Self {
        if self.const_lt(other) { self } else { other }
    }

    /// The larger of two values. `Math.max`.
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

    /// The average of two values, rounded toward zero, without an
    /// intermediate that overflows. `Math.average`.
    ///
    /// `(a + b) / 2` would wrap; `(a & b) + (a ^ b) / 2` is the same value and
    /// never leaves the range.
    #[inline]
    pub const fn average(self, other: Self) -> Self {
        self.const_bitand(other)
            .wrapping_add(self.const_bitxor(other).wrapping_shr(1))
    }

    // -----------------------------------------------------------------------
    // Division
    // -----------------------------------------------------------------------

    /// Division rounding toward positive infinity, returning `None` if `rhs`
    /// is zero. `Math.ceilDiv`.
    #[inline]
    pub const fn checked_ceil_div(self, rhs: Self) -> Option<Self> {
        if self.is_zero() {
            // Guard the `self - 1` below, and skip the division entirely.
            // Also the one input for which the divisor being zero is *not*
            // reported -- so check that first.
            if rhs.is_zero() {
                return None;
            }
            return Some(Self::ZERO);
        }
        // `self >= 1`, so `self - 1` cannot wrap, and the largest possible
        // result is `(MAX - 1) / 1 + 1 == MAX`: the `+ 1` cannot overflow
        // either.
        match self.wrapping_sub(Self::ONE).checked_div(rhs) {
            Some(q) => Some(q.wrapping_add(Self::ONE)),
            None => None,
        }
    }

    /// Division rounding toward positive infinity. `Math.ceilDiv`.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero, matching `/`.
    #[inline]
    pub const fn ceil_div(self, rhs: Self) -> Self {
        match self.checked_ceil_div(rhs) {
            Some(v) => v,
            None => panic!("attempt to divide by zero"),
        }
    }

    /// Division following `rounding`, returning `None` if `rhs` is zero.
    #[inline]
    pub const fn checked_div_rounding(
        self,
        rhs: Self,
        rounding: Rounding,
    ) -> Option<Self> {
        match self.checked_div_rem(rhs) {
            Some((q, r)) => {
                if rounding.unsigned_rounds_up() && !r.is_zero() {
                    Some(q.wrapping_add(Self::ONE))
                } else {
                    Some(q)
                }
            }
            None => None,
        }
    }

    /// Division following `rounding`.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero, matching `/`.
    #[inline]
    pub const fn div_rounding(self, rhs: Self, rounding: Rounding) -> Self {
        match self.checked_div_rounding(rhs, rounding) {
            Some(v) => v,
            None => panic!("attempt to divide by zero"),
        }
    }

    // -----------------------------------------------------------------------
    // Full-precision multiply-divide
    // -----------------------------------------------------------------------

    /// `floor(x * y / d)` computed on the exact 512-bit product, returning
    /// `None` if `d` is zero or the quotient exceeds [`U256::MAX`].
    ///
    /// `Math.mulDiv`. The intermediate never has to fit in 256 bits, which is
    /// the whole point: `x * y` may be far past [`U256::MAX`] as long as the
    /// quotient is not.
    #[inline]
    pub const fn checked_mul_div(x: Self, y: Self, d: Self) -> Option<Self> {
        match Self::mul_div_parts(x, y, d) {
            Some((q, _)) => Some(q),
            None => None,
        }
    }

    /// `floor(x * y / d)` computed on the exact 512-bit product. `Math.mulDiv`.
    ///
    /// # Panics
    ///
    /// If `d` is zero, or if the quotient does not fit in a `U256`. Both
    /// panics are unconditional rather than `overflow-checks`-gated, for the
    /// reason [`mul_div_u128`](crate::mul_div_u128) gives: this is an explicit
    /// exact-arithmetic call, not an operator with primitive behaviour to
    /// match.
    #[inline]
    pub const fn mul_div(x: Self, y: Self, d: Self) -> Self {
        match Self::checked_mul_div(x, y, d) {
            Some(v) => v,
            None => {
                if d.is_zero() {
                    panic!("attempt to divide by zero");
                }
                panic!("mul_div quotient exceeds U256");
            }
        }
    }

    /// `x * y / d` computed on the exact 512-bit product, following
    /// `rounding`. Returns `None` if `d` is zero or the result exceeds
    /// [`U256::MAX`]. `Math.mulDiv` with a rounding argument.
    #[inline]
    pub const fn checked_mul_div_rounding(
        x: Self,
        y: Self,
        d: Self,
        rounding: Rounding,
    ) -> Option<Self> {
        match Self::mul_div_parts(x, y, d) {
            Some((q, r)) => {
                if rounding.unsigned_rounds_up() && !r.is_zero() {
                    // Rounding up can leave the 256-bit range that the floor
                    // quotient just fit into.
                    q.checked_add(Self::ONE)
                } else {
                    Some(q)
                }
            }
            None => None,
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
                panic!("mul_div quotient exceeds U256");
            }
        }
    }

    /// The floor quotient and remainder of `x * y / d`, or `None` when `d` is
    /// zero or the quotient needs more than 256 bits.
    ///
    /// The one place the 512-bit product meets the divider; every public
    /// `mul_div`/`mul_mod` entry point is a thin wrapper on this.
    #[inline]
    pub(crate) const fn mul_div_parts(
        x: Self,
        y: Self,
        d: Self,
    ) -> Option<(Self, Self)> {
        let (lo, hi) = x.widening_mul_u256(y);
        let (q, r) = match div_rem_512(hi, lo, d) {
            Some(v) => v,
            None => return None,
        };
        // The quotient of a 512-bit dividend genuinely needs 512 bits when
        // `d <= hi`; that is the overflow case, not a representable answer.
        let mut i = LIMBS;
        while i < WIDE_LIMBS {
            if q[i] != 0 {
                return None;
            }
            i += 1;
        }
        Some((Self::from_limbs(low_limbs(&q)), r))
    }

    /// `floor(x * y >> n)` computed on the exact 512-bit product, returning
    /// `None` if the result exceeds [`U256::MAX`] or `n` is at least 256.
    ///
    /// `Math.mulShr`, whose `n` is a `uint8` and so cannot reach 256 at all.
    #[inline]
    pub const fn checked_mul_shr(x: Self, y: Self, n: u32) -> Option<Self> {
        match Self::mul_shr_parts(x, y, n) {
            Some((v, _)) => Some(v),
            None => None,
        }
    }

    /// The shifted product and whether any set bit was shifted out, or `None`
    /// when the result does not fit.
    ///
    /// One place the 512-bit product is formed, so the rounding entry point
    /// does not have to build it a second time -- `widening_mul_u256` is
    /// `inline(never)` on rv32 and sixty-four `mul`/`mulhu` pairs, which is
    /// not a thing to pay twice for a single `Ceil`.
    #[inline]
    const fn mul_shr_parts(x: Self, y: Self, n: u32) -> Option<(Self, bool)> {
        if n >= Self::BITS {
            return None;
        }
        let (lo, hi) = x.widening_mul_u256(y);
        // `hi >= 1 << n` is exactly `hi` needing more than `n` bits, spelled
        // without a shift that would have to special-case `n == 0`.
        if hi.bit_len() > n {
            return None;
        }
        if n == 0 {
            // Nothing is shifted out, so nothing is lost.
            return Some((lo, false));
        }
        // Only the low half can carry a dropped bit, since `n < 256`.
        let dropped = !lo
            .const_bitand(Self::MAX.wrapping_shr(Self::BITS - n))
            .is_zero();
        Some((
            hi.wrapping_shl(Self::BITS - n)
                .const_bitor(lo.wrapping_shr(n)),
            dropped,
        ))
    }

    /// `floor(x * y >> n)` computed on the exact 512-bit product.
    /// `Math.mulShr`.
    ///
    /// # Panics
    ///
    /// If `n` is at least 256, or if the result does not fit in a `U256`.
    #[inline]
    pub const fn mul_shr(x: Self, y: Self, n: u32) -> Self {
        match Self::checked_mul_shr(x, y, n) {
            Some(v) => v,
            None => {
                if n >= Self::BITS {
                    panic!("attempt to shift right with overflow");
                }
                panic!("mul_shr result exceeds U256");
            }
        }
    }

    /// `x * y >> n` computed on the exact 512-bit product, following
    /// `rounding`. `Math.mulShr` with a rounding argument.
    #[inline]
    pub const fn checked_mul_shr_rounding(
        x: Self,
        y: Self,
        n: u32,
        rounding: Rounding,
    ) -> Option<Self> {
        let (v, dropped) = match Self::mul_shr_parts(x, y, n) {
            Some(pair) => pair,
            None => return None,
        };
        if dropped && rounding.unsigned_rounds_up() {
            // Rounding up can leave the range the truncated value just fit.
            v.checked_add(Self::ONE)
        } else {
            Some(v)
        }
    }

    /// `x * y >> n` computed on the exact 512-bit product, following
    /// `rounding`.
    ///
    /// # Panics
    ///
    /// As [`mul_shr`](Self::mul_shr).
    #[inline]
    pub const fn mul_shr_rounding(
        x: Self,
        y: Self,
        n: u32,
        rounding: Rounding,
    ) -> Self {
        match Self::checked_mul_shr_rounding(x, y, n, rounding) {
            Some(v) => v,
            None => {
                if n >= Self::BITS {
                    panic!("attempt to shift right with overflow");
                }
                panic!("mul_shr result exceeds U256");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Modular arithmetic
    // -----------------------------------------------------------------------

    /// `(x + y) % m`, computed on the exact 257-bit sum. Returns `None` if `m`
    /// is zero.
    ///
    /// The EVM's `addmod`, which `Math.sol` uses but does not re-export.
    #[inline]
    pub const fn checked_add_mod(x: Self, y: Self, m: Self) -> Option<Self> {
        let (x, y) = match (x.checked_rem(m), y.checked_rem(m)) {
            (Some(x), Some(y)) => (x, y),
            _ => return None,
        };
        // Both are below `m`, so the true sum is below `2m` and one
        // conditional subtraction lands in range.
        let (sum, carry) = x.overflowing_add(y);
        if carry || sum.const_ge(m) {
            // On carry the true sum is `sum + 2^256`, and `true - m` is below
            // `2^256`, so the wrapping subtraction is exact.
            Some(sum.wrapping_sub(m))
        } else {
            Some(sum)
        }
    }

    /// `(x + y) % m`, computed on the exact 257-bit sum.
    ///
    /// # Panics
    ///
    /// If `m` is zero.
    #[inline]
    pub const fn add_mod(x: Self, y: Self, m: Self) -> Self {
        match Self::checked_add_mod(x, y, m) {
            Some(v) => v,
            None => panic!("attempt to divide by zero"),
        }
    }

    /// `(x * y) % m`, computed on the exact 512-bit product. Returns `None` if
    /// `m` is zero.
    ///
    /// The EVM's `mulmod`, which `Math.sol` uses but does not re-export.
    #[inline]
    pub const fn checked_mul_mod(x: Self, y: Self, m: Self) -> Option<Self> {
        let (lo, hi) = x.widening_mul_u256(y);
        match div_rem_512(hi, lo, m) {
            Some((_, r)) => Some(r),
            None => None,
        }
    }

    /// `(x * y) % m`, computed on the exact 512-bit product.
    ///
    /// # Panics
    ///
    /// If `m` is zero.
    #[inline]
    pub const fn mul_mod(x: Self, y: Self, m: Self) -> Self {
        match Self::checked_mul_mod(x, y, m) {
            Some(v) => v,
            None => panic!("attempt to divide by zero"),
        }
    }

    /// `b.pow(e) % m`, returning `None` if `m` is zero. `Math.tryModExp`.
    ///
    /// The zero-modulus refusal follows OpenZeppelin rather than the
    /// precompile it calls: EIP-198 treats a zero modulus as defined and
    /// answers zero, and `tryModExp` guards it in Solidity *before* the
    /// `staticcall`. A port is written against the library, so this matches
    /// the library.
    ///
    /// Square-and-multiply over [`checked_mul_mod`](Self::checked_mul_mod):
    /// one full-precision multiply-divide per bit of `e`, plus one more per
    /// set bit -- up to 512 for a full-width exponent. See
    /// [`mod_exp`](Self::mod_exp) for what that costs and why an
    /// attacker-chosen exponent is a gas-exhaustion surface.
    pub const fn checked_mod_exp(b: Self, e: Self, m: Self) -> Option<Self> {
        if m.is_zero() {
            return None;
        }
        if m.const_eq(Self::ONE) {
            // Everything is congruent to zero, including `0^0`.
            return Some(Self::ZERO);
        }

        let mut result = Self::ONE;
        let mut base = match b.checked_rem(m) {
            Some(v) => v,
            None => return None,
        };

        let bits = e.bit_len();
        let mut i = 0;
        while i < bits {
            if e.bit(i) {
                result = match Self::checked_mul_mod(result, base, m) {
                    Some(v) => v,
                    None => return None,
                };
            }
            i += 1;
            // Skip the final squaring: nothing reads it.
            if i < bits {
                base = match Self::checked_mul_mod(base, base, m) {
                    Some(v) => v,
                    None => return None,
                };
            }
        }
        Some(result)
    }

    /// `b.pow(e) % m`. `Math.modExp`.
    ///
    /// # Cost
    ///
    /// **This is the most expensive function in the crate, by a wide margin,
    /// and the one place where being a library rather than a precompile
    /// really costs.**
    ///
    /// `Math.modExp` in Solidity is not an implementation: it is a
    /// `staticcall` to the EIP-198 precompile at address `0x05`, so the
    /// bignum runs as native client code at a flat fee. Thruput's syscall
    /// table -- the actual analogue of the EVM's precompile slots -- carries
    /// `kSHA256` and `kVerifySignature` (EVM `0x02` and `0x01`) but nothing
    /// for modular exponentiation, so this runs *inside* the contract.
    ///
    /// That means the loop is real work: once per bit of `e`, with one
    /// full-precision multiply-divide per bit and another per set bit, up to
    /// 512 of them for a full-width exponent. At the 4,712 gas the bench
    /// measures for one `mul_div`, a 256-bit exponent is on the order of two
    /// million. An entrypoint that lets a caller choose `e` or `m` is
    /// therefore a gas-exhaustion surface: bound the exponent, or charge for
    /// it.
    ///
    /// If modular exponentiation ever becomes load-bearing here -- RSA
    /// verification, anything with a pairing behind it -- the answer is a host
    /// syscall alongside `kSHA256`, not a better loop. That is a protocol
    /// change rather than an SDK one, and it is tracked as
    /// <https://github.com/thruputchain/thruput-sdk-rs/issues/383>.
    ///
    /// Two things this does have over the Solidity version. It is a
    /// `const fn`, where `Math.modExp` must be `view` because a `staticcall`
    /// is not `pure`. And it cannot silently answer zero: OpenZeppelin's own
    /// documentation warns that on a chain without the precompile the
    /// `staticcall` succeeds and the result "may be incorrectly interpreted as
    /// 0", which is not a failure mode an in-contract loop has.
    ///
    /// # Panics
    ///
    /// If `m` is zero.
    pub const fn mod_exp(b: Self, e: Self, m: Self) -> Self {
        match Self::checked_mod_exp(b, e, m) {
            Some(v) => v,
            None => panic!("attempt to divide by zero"),
        }
    }

    /// The modular multiplicative inverse of `a` in `Z/nZ`, or zero when `a`
    /// has no inverse (including when `n` is zero or one). `Math.invMod`.
    ///
    /// Iterative extended Euclid. The Bézout coefficient is carried as a
    /// magnitude with an implied alternating sign, rather than as a signed
    /// value: consecutive coefficients always have opposite signs, so the
    /// recurrence on magnitudes is `t[k+1] = t[k-1] + q * t[k]`, and the
    /// standard bound `t <= n / gcd` keeps every intermediate inside a
    /// `U256`.
    pub const fn inv_mod(a: Self, n: Self) -> Self {
        if n.is_zero() {
            return Self::ZERO;
        }

        // (r0, r1) = (r[k-1], r[k]) and (t0, t1) = (|t[k-1]|, |t[k]|), k = 1.
        let mut r0 = n;
        let mut r1 = match a.checked_rem(n) {
            Some(v) => v,
            None => return Self::ZERO,
        };
        let mut t0 = Self::ZERO;
        let mut t1 = Self::ONE;
        // `t[k]` is positive for odd `k`, negative for even `k`.
        let mut k_odd = true;

        while !r1.is_zero() {
            let (q, r) = match r0.checked_div_rem(r1) {
                Some(v) => v,
                None => return Self::ZERO,
            };
            r0 = r1;
            r1 = r;
            // Bounded above by `n`, per the identity `|t[final + 1]| = n / gcd`
            // -- so the wrapping forms never actually wrap.
            let t2 = t0.wrapping_add(q.wrapping_mul(t1));
            t0 = t1;
            t1 = t2;
            k_odd = !k_odd;
        }

        // `r0` is now the gcd and `t0` the coefficient of `a`, at index k - 1.
        if !r0.const_eq(Self::ONE) {
            return Self::ZERO;
        }
        // Index `k - 1` is odd -- so the coefficient is positive -- exactly
        // when `k` is even, which is when `k_odd` is now false.
        if !k_odd || t0.is_zero() {
            // `t0` is zero only for `n == 1`, where the inverse is zero too.
            t0
        } else {
            n.wrapping_sub(t0)
        }
    }

    /// The modular multiplicative inverse of `a` in `Z/pZ` for a prime `p`
    /// greater than two. `Math.invModPrime`.
    ///
    /// Fermat's little theorem: `a^(p-2) = a^-1 (mod p)`. **`p` is not checked
    /// for primality**; a composite modulus produces a wrong answer rather
    /// than an error, exactly as in Solidity. Use
    /// [`inv_mod`](Self::inv_mod) when the modulus is not known to be prime.
    ///
    /// # Cost
    ///
    /// This is [`mod_exp`](Self::mod_exp) with a full-width exponent, so it is
    /// as expensive as that function's worst case. It reads like a cheap
    /// helper and is not one.
    ///
    /// # Panics
    ///
    /// If `p` is zero. Note that this differs from [`inv_mod`](Self::inv_mod),
    /// which is total and answers zero for a degenerate modulus: swapping one
    /// for the other after establishing primality elsewhere turns that zero
    /// into an abort.
    pub const fn inv_mod_prime(a: Self, p: Self) -> Self {
        Self::mod_exp(a, p.wrapping_sub(Self::from_u32(2)), p)
    }

    // -----------------------------------------------------------------------
    // Exponentiation
    // -----------------------------------------------------------------------

    /// Exponentiation, with a flag reporting whether it wrapped.
    ///
    /// `0.pow(0)` is one, matching the primitives.
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
            // Skip the final squaring: nothing reads it, and squaring an
            // already-large base would report an overflow the result does not
            // have.
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

    /// Exponentiation, saturating at [`U256::MAX`].
    #[inline]
    pub const fn saturating_pow(self, exp: u32) -> Self {
        match self.overflowing_pow(exp) {
            (v, false) => v,
            (_, true) => Self::MAX,
        }
    }

    /// Exponentiation.
    ///
    /// # Panics
    ///
    /// On overflow, if `overflow-checks` is on -- otherwise it wraps, like the
    /// operators and like the primitives' `pow`.
    #[inline]
    pub const fn pow(self, exp: u32) -> Self {
        let (v, overflow) = self.overflowing_pow(exp);
        if crate::arith::OVERFLOW_CHECKS && overflow {
            panic!("attempt to multiply with overflow");
        }
        v
    }

    // -----------------------------------------------------------------------
    // Roots and logarithms
    // -----------------------------------------------------------------------

    /// The integer square root, following `rounding`. `Math.sqrt` with a
    /// rounding argument.
    #[inline]
    pub const fn sqrt_rounding(self, rounding: Rounding) -> Self {
        let r = self.integer_sqrt();
        // `r <= 2^128 - 1`, so `r * r` cannot overflow and `r + 1` cannot
        // either.
        if rounding.unsigned_rounds_up() && !r.wrapping_mul(r).const_eq(self) {
            r.wrapping_add(Self::ONE)
        } else {
            r
        }
    }

    /// `floor(log2(self))`, or zero for zero. `Math.log2`.
    #[inline]
    pub const fn log2(self) -> u32 {
        self.bit_len().saturating_sub(1)
    }

    /// `log2(self)` following `rounding`, or zero for zero. `Math.log2` with a
    /// rounding argument.
    #[inline]
    pub const fn log2_rounding(self, rounding: Rounding) -> u32 {
        let r = self.log2();
        if rounding.unsigned_rounds_up()
            && Self::ONE.wrapping_shl(r).const_lt(self)
        {
            r + 1
        } else {
            r
        }
    }

    /// `floor(log10(self))`, or zero for zero. `Math.log10`.
    ///
    /// Peels eight digits at a time with [`div_rem_small`](Self::div_rem_small)
    /// -- `10^8` is the largest power of ten that is both a `u32` and a whole
    /// number of digits -- so this never links Knuth D, unlike the ladder of
    /// 256-bit constants Solidity uses.
    pub const fn log10(self) -> u32 {
        self.log10_parts().0
    }

    /// `floor(log10(self))`, and whether `10^that` is strictly below `self`.
    ///
    /// The second value is what [`log10_rounding`](Self::log10_rounding)
    /// needs, and it falls out of this loop for free: the result is a power of
    /// ten exactly when every peeled chunk divided exactly and what is left is
    /// itself one. Recomputing `10^r` afterwards to compare would instead link
    /// [`pow`](Self::pow) -- and through it the general 256x256 multiply --
    /// into a function built specifically to avoid dragging in that class of
    /// code.
    const fn log10_parts(self) -> (u32, bool) {
        /// The largest power of ten fitting in a `u32`'s divisor slot.
        const P8: u32 = 100_000_000;

        // `log10(0)` is defined as 0 here, and `10^0 == 1` is above zero
        // rather than below it, so zero never rounds up. Every path below
        // assumes `self >= 1`.
        if self.is_zero() {
            return (0, false);
        }

        let mut v = self;
        let mut r = 0u32;
        let mut exact = true;
        // `limb_len() > 1` means `v >= 2^32 > 10^8`, so the second test only
        // ever runs on a single-limb value.
        while v.limb_len() > 1 || v.to_limbs()[0] >= P8 {
            let (q, rem) = v.div_rem_small(P8);
            if rem != 0 {
                exact = false;
            }
            v = q;
            r += 8;
        }
        let (tail, tail_exact) = log10_u32(v.to_limbs()[0]);
        (r + tail, !(exact && tail_exact))
    }

    /// `log10(self)` following `rounding`, or zero for zero. `Math.log10` with
    /// a rounding argument.
    #[inline]
    pub const fn log10_rounding(self, rounding: Rounding) -> u32 {
        let (r, inexact) = self.log10_parts();
        if rounding.unsigned_rounds_up() && inexact {
            r + 1
        } else {
            r
        }
    }

    /// `floor(log256(self))`, or zero for zero. `Math.log256`.
    ///
    /// One more than this is the number of bytes needed to write `self`.
    #[inline]
    pub const fn log256(self) -> u32 {
        self.bit_len().saturating_sub(1) / 8
    }

    /// `log256(self)` following `rounding`, or zero for zero. `Math.log256`
    /// with a rounding argument.
    #[inline]
    pub const fn log256_rounding(self, rounding: Rounding) -> u32 {
        let r = self.log256();
        if rounding.unsigned_rounds_up()
            && Self::ONE.wrapping_shl(r * 8).const_lt(self)
        {
            r + 1
        } else {
            r
        }
    }
}

/// `floor(log10(x))`, and whether `x` is exactly that power of ten.
///
/// `x` is at least 1 and below `10^8` at the only call site, which is what
/// lets the final value decide exactness: after the ladder it is a single
/// digit, and a power of ten only if that digit is 1.
#[inline]
const fn log10_u32(x: u32) -> (u32, bool) {
    let mut x = x;
    let mut r = 0;
    let mut exact = true;
    if x >= 10_000 {
        exact &= x.is_multiple_of(10_000);
        x /= 10_000;
        r += 4;
    }
    if x >= 100 {
        exact &= x.is_multiple_of(100);
        x /= 100;
        r += 2;
    }
    if x >= 10 {
        exact &= x.is_multiple_of(10);
        x /= 10;
        r += 1;
    }
    (r, exact && x == 1)
}
