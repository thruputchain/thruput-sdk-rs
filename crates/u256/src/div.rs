//! Division and remainder.
//!
//! [`U256::div_rem_small`] is a separate function rather than a fast path
//! inside [`U256::div_rem`], so a contract that only divides by constants (or
//! formats a decimal) links the cheap routine and not Knuth D. Measured on an
//! rv32 probe linking one call and nothing else: the general path costs 1,576
//! bytes of text on top of the small one. (It was 2,836 before the quotient
//! estimate stopped going through `__udivdi3` -- see the estimate below.)

use crate::{LIMBS, U256};

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
    // half-digit estimates below.
    let s = d.leading_zeros();
    let d = d << s;
    let vn1 = d >> 16;
    let vn0 = d & 0xffff;

    let un32 = if s == 0 {
        hi
    } else {
        (hi << s) | (lo >> (32 - s))
    };
    let un10 = lo << s;
    let un1 = un10 >> 16;
    let un0 = un10 & 0xffff;

    // `rhat < 2^16` is held by the `break`, and `q1 * vn0` is only reached once
    // `q1 < 2^16`, so neither overflows.
    let mut q1 = un32 / vn1;
    let mut rhat = un32 - q1 * vn1;
    while q1 >= HB || q1 * vn0 > (rhat << 16) | un1 {
        q1 -= 1;
        rhat += vn1;
        if rhat >= HB {
            break;
        }
    }

    // `un21` fits in 32 bits but `un32 * 2^16` does not; the wrapping is
    // deliberate and lands on the right answer.
    let un21 = un32
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
        .wrapping_sub(q0.wrapping_mul(d))
        >> s;

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
        if rhs == 0 {
            panic!("attempt to divide by zero");
        }

        let mut q = [0u32; LIMBS];
        let mut rem = 0u32;

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
            let mut rem = [0u32; LIMBS];
            rem[0] = r;
            return Some((q, Self::from_limbs(rem)));
        }

        // Knuth algorithm D (Hacker's Delight `divmnu`), base 2^32.
        let u = self.limbs;
        let v = rhs.limbs;

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
        let mut un = [0u32; LIMBS + 1];
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

        let mut q = [0u32; LIMBS];

        let mut j = nn - dn + 1;
        while j > 0 {
            j -= 1;

            // Estimate one quotient limb from the top two dividend limbs.
            //
            // `div_rem_2by1` wants `hi < dv`, while Knuth allows equality (the
            // running remainder is below the divisor, so `hi <= dv`). Equality
            // means the true digit is `B + lo / dv`, at least `B`; step D3
            // clamps that to `B - 1`, which is still an upper bound on the real
            // quotient digit, and the correction below plus the add-back take
            // it the rest of the way. Spelling the common branch as
            // `div_rem_2by1` keeps the whole loop on native 32-bit `DIVU`s
            // instead of one `__udivdi3` per quotient digit.
            let hi = un[j + dn];
            let lo = un[j + dn - 1];
            let dv = vn[dn - 1];

            let (mut qhat, mut rhat) = if hi < dv {
                let (qh, rh) = div_rem_2by1(hi, lo, dv);
                (qh as u64, rh as u64)
            } else {
                let num = ((hi as u64) << 32) | (lo as u64);
                (B - 1, num - (B - 1) * (dv as u64))
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
                let t = (un[i + j] as i64) - k - ((p & 0xffff_ffff) as i64);
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
                un[j + dn] = (un[j + dn] as u64).wrapping_add(carry) as u32;
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

        Some((Self::from_limbs(q), Self::from_limbs(r)))
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
