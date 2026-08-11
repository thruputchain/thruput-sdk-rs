//! Division and remainder.
//!
//! # Why the small-divisor path is a separate function
//!
//! Knuth algorithm D is the expensive item in this crate -- `uint`'s
//! equivalent measured 11,300 bytes of rv32 text, 17% of the artifact
//! format's 64 KiB code cap. Dividing by a small constant is far more common
//! than dividing by another 256-bit value, and decimal formatting divides by
//! `10^9` repeatedly.
//!
//! So [`U256::div_rem_small`] is its own function rather than a fast path
//! inside [`U256::div_rem`]. A contract that only ever divides by constants
//! links the cheap routine and nothing else; only reaching for the general
//! `U256 / U256` pulls Knuth D in.

use crate::{LIMBS, U256};

/// The base of the limb representation, as a `u64` for intermediate work.
const B: u64 = 1 << 32;

/// Half-limb base, for the 64-by-32 divide below.
const HB: u32 = 1 << 16;

/// Divides the 64-bit value `(hi, lo)` by `d`, returning `(quotient,
/// remainder)`. Requires `hi < d` (so the quotient fits in 32 bits) and
/// `d != 0`; both hold at every call site.
///
/// # Why not just write `((hi as u64) << 32 | lo as u64) / d as u64`
///
/// Because on riscv32 that is not a division instruction, it is a call to
/// `__udivdi3`/`__umoddi3`. `DIVU` is XLEN-wide, so on a 32-bit target it
/// divides 32-bit operands only — there is no 64-by-32 divide to lower onto.
/// Written the obvious way, `div_rem_small` paid a compiler-builtins call on
/// every one of its eight limb iterations, which is precisely the cost this
/// crate exists to remove. Verified by disassembly: the u64 spelling produced
/// zero `divu` instructions.
///
/// This is Hacker's Delight's `divlu`, schoolbook division in base 2^16 so
/// that every divide is a native 32-bit `DIVU`.
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

    // First half-digit. `rhat < 2^16` is maintained by the `break`, so
    // `rhat << 16` never overflows; and `q1 * vn0` is only evaluated once
    // `q1 < 2^16` (the `||` short-circuits), so it cannot overflow either.
    let mut q1 = un32 / vn1;
    let mut rhat = un32 - q1 * vn1;
    while q1 >= HB || q1 * vn0 > (rhat << 16) | un1 {
        q1 -= 1;
        rhat += vn1;
        if rhat >= HB {
            break;
        }
    }

    // The true value of `un21` fits in 32 bits; the intermediate
    // `un32 * 2^16` does not, so this is deliberately modular. Two's
    // complement makes the wrapped arithmetic land on the right answer.
    let un21 = un32
        .wrapping_mul(HB)
        .wrapping_add(un1)
        .wrapping_sub(q1.wrapping_mul(d));

    // Second half-digit.
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
            let (qi, r) = div_rem_2by1(rem, self.0[i], rhs);
            q[i] = qi;
            rem = r;
        }

        (Self(q), rem)
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

        // Single-limb divisor: hand off to the cheap routine. This is a call
        // to a separate function, not an inlined copy, so a contract that only
        // uses `div_rem_small` never links the code below it.
        if dn == 1 {
            let (q, r) = self.div_rem_small(rhs.0[0]);
            let mut rem = [0u32; LIMBS];
            rem[0] = r;
            return Some((q, Self(rem)));
        }

        // Knuth algorithm D (Hacker's Delight `divmnu`), base 2^32.
        let u = self.0;
        let v = rhs.0;

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
            let num = ((un[j + dn] as u64) << 32) | (un[j + dn - 1] as u64);
            let mut qhat = num / (vn[dn - 1] as u64);
            let mut rhat = num % (vn[dn - 1] as u64);

            while qhat >= B
                || qhat * (vn[dn - 2] as u64)
                    > (rhat << 32) | (un[j + dn - 2] as u64)
            {
                qhat -= 1;
                rhat += vn[dn - 1] as u64;
                if rhat >= B {
                    break;
                }
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

            // The estimate was one too large (probability ~2/2^32): add the
            // divisor back. Rare enough that the differential tests need a
            // wide random sweep to exercise it at all.
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

        Some((Self(q), Self(r)))
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

    /// The reference is the spelling `div_rem_2by1` replaced: a plain u64
    /// divide. It is correct and obvious; it is just a libcall on rv32.
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
