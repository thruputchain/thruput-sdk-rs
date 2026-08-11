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

impl U256 {
    /// Divides by a 32-bit value, returning the quotient and remainder.
    ///
    /// Each step is a 64-by-32 divide, which is what rv32's `DIVU` natively
    /// performs.
    ///
    /// # Panics
    ///
    /// If `rhs` is zero.
    #[inline]
    pub const fn div_rem_small(self, rhs: u32) -> (Self, u32) {
        if rhs == 0 {
            panic!("attempt to divide by zero");
        }

        let d = rhs as u64;
        let mut q = [0u32; LIMBS];
        let mut rem = 0u64;

        let mut i = LIMBS;
        while i > 0 {
            i -= 1;
            let cur = (rem << 32) | (self.0[i] as u64);
            q[i] = (cur / d) as u32;
            rem = cur % d;
        }

        (Self(q), rem as u32)
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
