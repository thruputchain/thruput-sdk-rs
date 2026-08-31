//! Division and remainder.
//!
//! [`U256::div_rem_small`] is a separate function rather than a fast path
//! inside [`U256::div_rem`], so a contract that only divides by constants (or
//! formats a decimal) links the cheap routine and not Knuth D. Measured by
//! `scripts/bigint-bench.sh -b u256 -o "div div_small"`: linking `div` adds
//! 2,572 bytes of rv32 .text where `div_small` adds 680, so the general path
//! costs 1,892 on top of the small one. (It was 240 more before the Knuth D
//! loop called `div_rem_2by1_normalized` directly, and far worse when the
//! quotient estimate went through `__udivdi3` -- see the estimate below.)
//!
//! Knuth D is written once as a macro and instantiated at two dividend widths
//! -- eight limbs for the `U256 / U256` operators, sixteen for the
//! full-precision `mul_div` family, whose dividend is a 512-bit product. A
//! single shared function over the wider buffers was measured and rejected;
//! [`knuth_d`] has the numbers.

use crate::{LIMBS, U256};

/// The number of 32-bit limbs in a full-precision (512-bit) product.
pub(crate) const WIDE_LIMBS: usize = 2 * LIMBS;

/// The base of the limb representation, as a `u64` for intermediate work.
const B: u64 = 1 << 32;

/// Half-limb base, for the 64-by-32 divide below.
const HB: u32 = 1 << 16;

/// Divides `(hi, lo)` by `d`. Requires `hi < d` and `d != 0`; both hold at
/// every call site.
///
/// Hacker's Delight's `divlu` -- schoolbook base 2^16, so every divide is a
/// native 32-bit `DIVU`. The obvious `(hi:lo as u64) / d` spelling is not:
/// `DIVU` is XLEN-wide, so on rv32 it lowers to `__udivdi3`/`__umoddi3`, one
/// libcall per limb. (Verified by disassembly; it emitted zero `divu`.)
#[inline]
const fn div_rem_2by1(hi: u32, lo: u32, d: u32) -> (u32, u32) {
    debug_assert!(hi < d);

    // Normalise so the divisor's high bit is set, which is what bounds the
    // half-digit estimates in the core.
    let s = d.leading_zeros();
    let un32 = if s == 0 {
        hi
    } else {
        (hi << s) | (lo >> (32 - s))
    };
    let (q, rem) = div_rem_2by1_normalized(un32, lo << s, d << s);
    (q, rem >> s)
}

/// The core of [`div_rem_2by1`], for a divisor whose high bit is already set.
/// Requires `hi < d`.
///
/// Split out so the Knuth D loop, which normalises its divisor once up front,
/// does not re-derive `s == 0` per quotient digit. LLVM cannot prove it --
/// known-bits does not carry `x << clz(x)` through the array stores -- so the
/// wrapper's selects and shifts would otherwise ride along in the hot loop as
/// dead work.
#[inline]
const fn div_rem_2by1_normalized(hi: u32, lo: u32, d: u32) -> (u32, u32) {
    debug_assert!(hi < d);
    debug_assert!(d >> 31 == 1);

    let vn1 = d >> 16;
    let vn0 = d & 0xffff;

    let un1 = lo >> 16;
    let un0 = lo & 0xffff;

    // `rhat < 2^16` is held by the `break`, and `q1 * vn0` is only reached once
    // `q1 < 2^16`, so neither overflows.
    let mut q1 = hi / vn1;
    let mut rhat = hi - q1 * vn1;
    while q1 >= HB || q1 * vn0 > (rhat << 16) | un1 {
        q1 -= 1;
        rhat += vn1;
        if rhat >= HB {
            break;
        }
    }

    // `un21` fits in 32 bits but `hi * 2^16` does not; the wrapping is
    // deliberate and lands on the right answer.
    let un21 = hi
        .wrapping_mul(HB)
        .wrapping_add(un1)
        .wrapping_sub(q1.wrapping_mul(d));

    let mut q0 = un21 / vn1;
    rhat = un21 - q0 * vn1;
    while q0 >= HB || q0 * vn0 > (rhat << 16) | un0 {
        q0 -= 1;
        rhat += vn1;
        if rhat >= HB {
            break;
        }
    }

    let rem = un21
        .wrapping_mul(HB)
        .wrapping_add(un0)
        .wrapping_sub(q0.wrapping_mul(d));

    (q1 * HB + q0, rem)
}

impl U256 {
    /// Divides by a 32-bit value, returning the quotient and remainder.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn div_rem_small(self, rhs: u32) -> (Self, u32) {
        self.div_rem_small_from(rhs, 0)
    }

    /// [`div_rem_small`](Self::div_rem_small), resuming from a partial
    /// remainder rather than from zero.
    ///
    /// Chaining two calls divides a 512-bit value by a 32-bit one: the high
    /// half's remainder seeds the low half. That keeps the `dn == 1` case of
    /// [`div_rem_512`] on this cheap routine instead of widening the Knuth D
    /// buffers to reach it.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero. `rem` must already be below `rhs`, which every
    /// caller satisfies by construction (it is a remainder of `rhs`).
    #[inline]
    pub(crate) const fn div_rem_small_from(
        self,
        rhs: u32,
        rem: u32,
    ) -> (Self, u32) {
        if rhs == 0 {
            panic!("attempt to divide by zero");
        }
        debug_assert!(rem < rhs);

        let mut q = [0u32; LIMBS];
        let mut rem = rem;

        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            // `rem` is a remainder, so `rem < rhs` holds on every iteration,
            // which is exactly `div_rem_2by1`'s precondition.
            let (qi, r) = div_rem_2by1(rem, self.limbs[i], rhs);
            q[i] = qi;
            rem = r;
        }

        (Self::from_limbs(q), rem)
    }

    /// Divides by another `U256`, returning the quotient and remainder.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero, matching the primitive integers (which panic on a
    /// zero divisor regardless of `overflow-checks`).
    pub const fn div_rem(self, rhs: Self) -> (Self, Self) {
        match self.checked_div_rem(rhs) {
            Some(pair) => pair,
            None => panic!("attempt to divide by zero"),
        }
    }

    /// Divides by another `U256`, returning `None` if `rhs` is zero.
    pub const fn checked_div_rem(self, rhs: Self) -> Option<(Self, Self)> {
        let dn = rhs.limb_len();
        if dn == 0 {
            return None;
        }
        let nn = self.limb_len();

        // Divisor larger than the dividend: quotient zero, remainder self.
        if nn < dn {
            return Some((Self::ZERO, self));
        }

        // Separate function, not an inlined copy, so a contract using only
        // `div_rem_small` never links the code below.
        if dn == 1 {
            let (q, r) = self.div_rem_small(rhs.limbs[0]);
            return Some((q, Self::from_u32(r)));
        }

        let (q, r) = div_rem_knuth(&self.limbs, nn, &rhs.limbs, dn);
        Some((Self::from_limbs(q), r))
    }
    /// Division, returning `None` if `rhs` is zero.
    #[inline]
    pub const fn checked_div(self, rhs: Self) -> Option<Self> {
        match self.checked_div_rem(rhs) {
            Some((q, _)) => Some(q),
            None => None,
        }
    }

    /// Remainder, returning `None` if `rhs` is zero.
    #[inline]
    pub const fn checked_rem(self, rhs: Self) -> Option<Self> {
        match self.checked_div_rem(rhs) {
            Some((_, r)) => Some(r),
            None => None,
        }
    }
}

/// The low [`LIMBS`] limbs of a full-precision limb array.
#[inline]
pub(crate) const fn low_limbs(wide: &[u32; WIDE_LIMBS]) -> [u32; LIMBS] {
    let mut out = [0u32; LIMBS];
    let mut i = 0;
    while i < LIMBS {
        out[i] = wide[i];
        i += 1;
    }
    out
}

/// Generates a Knuth algorithm D (Hacker's Delight `divmnu`) specialised to a
/// dividend of `$n` limbs, base 2^32.
///
/// The generated function divides the `nn`-limb value `u` by the `dn`-limb
/// value `v`, returning the quotient's limbs and the remainder. The quotient
/// occupies the low `nn - dn + 1` limbs of the returned array; the rest are
/// zero.
///
/// It requires `2 <= dn <= LIMBS`, `dn <= nn <= $n`, `u[nn - 1] != 0` and
/// `v[dn - 1] != 0`. Every caller establishes these from a `limb_len`.
///
/// # Why a macro
///
/// Two widths need this: the `U256 / U256` operators divide eight limbs, and
/// [`div_rem_512`] divides the sixteen of a full-precision product. One body,
/// two instantiations, each sized to its own dividend.
///
/// The obvious alternative -- one function over the wider buffers, called by
/// both, since every loop is bounded by the runtime `nn`/`dn` anyway -- was
/// written first and measured second. It loses.
/// `scripts/bigint-bench.sh -b u256 -o "div div_small mul_div_u128"`, rv32,
/// marginal over the `none` calibration build:
///
/// | | `div` .text | `div` gas | `mul_div_u128` .text | `mul_div_u128` gas |
/// |---|---|---|---|---|
/// | before this change | 2,640 | 963.05 | 3,036 | 1735.05 |
/// | one shared function | 2,576 | 1221.05 | 2,996 | 1798.05 |
/// | two instantiations | 2,572 | 972.05 | 3,012 | 1712.05 |
///
/// Sharing one function buys 64 bytes and costs **+258 gas per division, 27% on
/// the bench's operands** -- the zero-init and copy-out of buffers twice the
/// size the narrow path can ever fill. Division is the hot path of the
/// contracts this crate exists for, so that is the wrong way round.
/// Specialising gets the bytes anyway (68 below the pre-change baseline) for
/// +0.9% gas, and leaves `mul_div_u128` both smaller and faster than before.
///
/// Read the 27% as a worst case, not a constant. The overhead is per call, so
/// it is a larger fraction of a shallow division than a deep one, and the
/// bench's operands make `div` shallow: its divisor is 201 bits, seven of
/// eight limbs, so Knuth D runs its quotient loop twice out of a possible
/// seven. It is also not purely fixed -- the same change cost `mul_div_u128`
/// only 63 gas through the same `checked_div_rem`, because LLVM elides more of
/// the oversized zero-init in that call shape. What holds across every row is
/// the sign: specialised is never worse than shared.
///
/// `#[inline]` on the generated function was measured too, on the theory that
/// it would restore the pre-split shape exactly: it costs 156 bytes on `div`
/// to save 2 gas, and makes `mul_div_u128` both bigger and slower. Not taken.
///
/// What the split costs instead: a contract using both `/` and the
/// full-precision family links Knuth D twice. That cost is *not* in the table
/// above -- the bench builds one kernel per artifact, so no row there carries
/// both instantiations. Its order of magnitude is the 1,892 bytes one copy
/// costs over `div_small`, inferred rather than measured. The trade was made on
/// the gas column, which is measured.
macro_rules! knuth_d {
    ($name:ident, $n:expr) => {
        const fn $name(
            u: &[u32; $n],
            nn: usize,
            v: &[u32; LIMBS],
            dn: usize,
        ) -> ([u32; $n], U256) {
            debug_assert!(dn >= 2);
            debug_assert!(nn >= dn);
            debug_assert!(nn <= $n);

            let s = v[dn - 1].leading_zeros();

            // Normalise the divisor so its top limb has its high bit set, which is
            // what bounds the quotient-digit estimate below.
            let mut vn = [0u32; LIMBS];
            let mut i = dn;
            while i > 0 {
                i -= 1;
                let lo = if i == 0 || s == 0 {
                    0
                } else {
                    v[i - 1] >> (32 - s)
                };
                vn[i] = (v[i] << s) | lo;
            }

            // Normalised dividend, one limb wider so the estimate always has two
            // limbs to read.
            let mut un = [0u32; $n + 1];
            un[nn] = if s == 0 { 0 } else { u[nn - 1] >> (32 - s) };
            let mut i = nn;
            while i > 0 {
                i -= 1;
                let lo = if i == 0 || s == 0 {
                    0
                } else {
                    u[i - 1] >> (32 - s)
                };
                un[i] = (u[i] << s) | lo;
            }

            let mut q = [0u32; $n];

            let mut j = nn - dn + 1;
            while j > 0 {
                j -= 1;

                // Estimate one quotient limb from the top two dividend limbs.
                //
                // `div_rem_2by1_normalized` wants `hi < dv`, while Knuth allows
                // equality (the running remainder is below the divisor, so
                // `hi <= dv`). Equality means the true digit is `B + lo / dv`, at
                // least `B`; step D3 clamps that to `B - 1`, which is still an
                // upper bound on the real quotient digit, and the correction below
                // plus the add-back take it the rest of the way. Spelling the
                // common branch as `div_rem_2by1_normalized` -- `dv` is the
                // normalised divisor's top limb, so its high bit is set -- keeps
                // the whole loop on native 32-bit `DIVU`s instead of one
                // `__udivdi3` per quotient digit, and skips the wrapper's
                // re-normalisation prologue.
                let hi = un[j + dn];
                let lo = un[j + dn - 1];
                let dv = vn[dn - 1];

                let (mut qhat, mut rhat) = if hi < dv {
                    let (qh, rh) = div_rem_2by1_normalized(hi, lo, dv);
                    (qh as u64, rh as u64)
                } else {
                    // `hi <= dv` (above), so only equality reaches here, and the
                    // Knuth rhat `num - (B - 1) * dv` collapses: `num` is
                    // `dv * B + lo`, so subtracting `(B - 1) * dv` leaves
                    // `dv + lo`.
                    debug_assert!(hi == dv);
                    (B - 1, (dv as u64) + (lo as u64))
                };

                // `rhat >= B` makes the test below vacuous -- `(rhat << 32) | ..`
                // exceeds any `qhat * vn[dn - 2]` -- and the shift would drop the
                // bits that make that true, so it guards the loop rather than only
                // breaking out of it. `qhat < B` now holds by construction, so the
                // estimate no longer needs its own bound check.
                while rhat < B
                    && qhat * (vn[dn - 2] as u64)
                        > (rhat << 32) | (un[j + dn - 2] as u64)
                {
                    qhat -= 1;
                    rhat += dv as u64;
                }

                // Multiply and subtract.
                let mut k: i64 = 0;
                let mut i = 0;
                while i < dn {
                    let p = qhat * (vn[i] as u64);
                    let t =
                        (un[i + j] as i64) - k - ((p & 0xffff_ffff) as i64);
                    un[i + j] = t as u32;
                    k = ((p >> 32) as i64) - (t >> 32);
                    i += 1;
                }
                let t = (un[j + dn] as i64) - k;
                un[j + dn] = t as u32;

                q[j] = qhat as u32;

                // Estimate one too large (probability ~2/2^32): add the divisor
                // back. Rare enough that the tests need a wide sweep to hit it.
                if t < 0 {
                    q[j] -= 1;
                    let mut carry = 0u64;
                    let mut i = 0;
                    while i < dn {
                        let sum = (un[i + j] as u64) + (vn[i] as u64) + carry;
                        un[i + j] = sum as u32;
                        carry = sum >> 32;
                        i += 1;
                    }
                    un[j + dn] =
                        (un[j + dn] as u64).wrapping_add(carry) as u32;
                }
            }

            // Undo the normalisation shift on the remainder.
            let mut r = [0u32; LIMBS];
            let mut i = 0;
            while i < dn {
                let hi = if s == 0 { 0 } else { un[i + 1] << (32 - s) };
                r[i] = (un[i] >> s) | hi;
                i += 1;
            }

            (q, U256::from_limbs(r))
        }
    };
}

knuth_d!(div_rem_knuth, LIMBS);
knuth_d!(div_rem_knuth_wide, WIDE_LIMBS);

/// Divides the 512-bit value `hi * 2^256 + lo` by `d`, returning the
/// quotient's sixteen limbs and the remainder. `None` if `d` is zero.
///
/// The quotient genuinely needs 512 bits when `d <= hi`; callers that require
/// a `U256` answer check the upper half themselves. The remainder is below
/// `d`, so it always fits.
pub(crate) const fn div_rem_512(
    hi: U256,
    lo: U256,
    d: U256,
) -> Option<([u32; WIDE_LIMBS], U256)> {
    let dn = d.limb_len();
    if dn == 0 {
        return None;
    }

    // Same split as the narrow path: dividing by a single limb stays on
    // `div_rem_small`, seeded across the two halves, rather than widening
    // Knuth D's buffers to reach a case it does not need.
    if dn == 1 {
        let divisor = d.limbs[0];
        let (qhi, rem) = hi.div_rem_small(divisor);
        let (qlo, rem) = lo.div_rem_small_from(divisor, rem);
        let mut q = [0u32; WIDE_LIMBS];
        let mut i = 0;
        while i < LIMBS {
            q[i] = qlo.limbs[i];
            q[i + LIMBS] = qhi.limbs[i];
            i += 1;
        }
        return Some((q, U256::from_u32(rem)));
    }

    let mut u = [0u32; WIDE_LIMBS];
    let mut i = 0;
    while i < LIMBS {
        u[i] = lo.limbs[i];
        u[i + LIMBS] = hi.limbs[i];
        i += 1;
    }

    let mut nn = WIDE_LIMBS;
    while nn > 0 && u[nn - 1] == 0 {
        nn -= 1;
    }
    // `dn <= LIMBS`, so a dividend with fewer significant limbs than the
    // divisor has an empty high half, and the remainder is just `lo`.
    if nn < dn {
        return Some(([0u32; WIDE_LIMBS], lo));
    }

    let (q, r) = div_rem_knuth_wide(&u, nn, &d.limbs, dn);
    Some((q, r))
}

/// Computes `a * b / d` exactly, without the intermediate product needing to
/// fit in 128 bits.
///
/// This is the case that sends contract authors looking for a 256-bit type in
/// the first place: two 128-bit amounts multiplied and scaled back down. The
/// `U256` never escapes into caller code.
///
/// # Panics
///
/// If `d` is zero, or if the quotient does not fit in a `u128`. The overflow
/// panic is unconditional rather than `overflow-checks`-gated: this is an
/// explicit API rather than an operator, and silently wrapping the result of a
/// deliberate exact-arithmetic call has no primitive behaviour to match.
#[inline]
pub fn mul_div_u128(a: u128, b: u128, d: u128) -> u128 {
    match checked_mul_div_u128(a, b, d) {
        Some(v) => v,
        None if d == 0 => panic!("attempt to divide by zero"),
        None => panic!("mul_div_u128 quotient exceeds u128"),
    }
}

/// Computes `a * b / d` exactly, returning `None` if `d` is zero or the
/// quotient does not fit in a `u128`.
#[inline]
pub fn checked_mul_div_u128(a: u128, b: u128, d: u128) -> Option<u128> {
    let (q, _) = U256::widening_mul(a, b).checked_div_rem(U256::from(d))?;
    u128::try_from(q).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spelling `div_rem_2by1` replaced -- correct and obvious, just a
    /// libcall on rv32.
    fn reference(hi: u32, lo: u32, d: u32) -> (u32, u32) {
        let n = ((hi as u64) << 32) | (lo as u64);
        ((n / d as u64) as u32, (n % d as u64) as u32)
    }

    fn check(hi: u32, lo: u32, d: u32) {
        assert!(hi < d, "precondition");
        assert_eq!(
            div_rem_2by1(hi, lo, d),
            reference(hi, lo, d),
            "({hi}, {lo}) / {d}"
        );
        // A normalised divisor also satisfies the core's stricter
        // precondition, so pin the core against the oracle directly rather
        // than only through the wrapper's `s == 0` path.
        if d >> 31 == 1 {
            assert_eq!(
                div_rem_2by1_normalized(hi, lo, d),
                reference(hi, lo, d),
                "normalized ({hi}, {lo}) / {d}"
            );
        }
    }

    #[test]
    fn div_rem_2by1_matches_u64_on_boundaries() {
        let interesting = [
            1u32,
            2,
            3,
            10,
            1_000_000_000,
            0xffff,
            0x1_0000,
            0x8000_0000,
            0xffff_fffe,
            u32::MAX,
        ];
        let los = [0u32, 1, 0xffff, 0x1_0000, 0x8000_0000, u32::MAX];

        for d in interesting {
            for lo in los {
                // hi must stay below d.
                for hi in [0, d / 2, d.saturating_sub(1)] {
                    if hi < d {
                        check(hi, lo, d);
                    }
                }
            }
        }
    }

    #[test]
    fn div_rem_2by1_matches_u64_on_random_inputs() {
        // Deterministic xorshift so a failure reproduces exactly.
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };

        for _ in 0..200_000 {
            let d = (next() as u32).max(1);
            let hi = (next() as u32) % d;
            let lo = next() as u32;
            check(hi, lo, d);
        }
    }

    /// Divisors with a set high bit skip normalisation (`s == 0`), which is
    /// the branch where a `>> (32 - s)` would be undefined.
    #[test]
    fn div_rem_2by1_handles_unnormalised_and_normalised_divisors() {
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };

        for _ in 0..50_000 {
            // Already normalised: high bit set, so s == 0.
            let d = (next() as u32) | 0x8000_0000;
            check((next() as u32) % d, next() as u32, d);

            // Maximally unnormalised: s == 31.
            check(0, next() as u32, 1);
        }
    }
}
