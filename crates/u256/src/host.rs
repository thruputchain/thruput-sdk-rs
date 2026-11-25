//! The runtime's arithmetic host operations, through their generated
//! bindings. Every division on these types is one of these calls: an `ecall`
//! in a contract, the native executor in a native test.
//!
//! A host program has neither. With the `software-arithmetic` feature, a
//! build that is not a contract computes the same results in software
//! instead (`soft`). On riscv32 the feature does nothing.
//!
//! The host reads the operands and writes the results in place. Each entry
//! point takes out only the results its caller uses: a whole result tuple
//! copied out of host-written memory compiles to a `memcpy` call.

use core::mem::MaybeUninit;

use crate::U256;

// The runtime reads and writes a 256-bit integer as 32 bytes, least
// significant first. On a little-endian target those are a `U256`'s own
// bytes.
const _: () = assert!(cfg!(target_endian = "little"));

/// `value`, as the 32 bytes the runtime reads.
#[cfg(any(not(feature = "software-arithmetic"), target_arch = "riscv32"))]
#[inline(always)]
pub(crate) fn bytes(value: &U256) -> &[u8; 32] {
    // SAFETY: `U256` is a transparent `[u32; 8]`: 32 bytes, aligned at least
    // as strictly as `[u8; 32]`.
    unsafe { &*core::ptr::from_ref(value).cast() }
}

/// `value`, as the 32 bytes the runtime writes.
#[cfg(any(not(feature = "software-arithmetic"), target_arch = "riscv32"))]
#[inline(always)]
fn bytes_mut(value: &mut MaybeUninit<U256>) -> &mut MaybeUninit<[u8; 32]> {
    // SAFETY: as for `bytes`, and any 32 bytes are a valid `U256`.
    unsafe { &mut *value.as_mut_ptr().cast() }
}

/// The three results of `U256MulDivRem`: the quotient's low and high 256 bits,
/// and the remainder.
struct Parts {
    lo: MaybeUninit<U256>,
    hi: MaybeUninit<U256>,
    rem: MaybeUninit<U256>,
}

/// `x * y` divided by `d`, by `U256MulDivRem`: what `take` reads from the
/// results.
///
/// # Panics
///
/// If the host operation fails, which it does for a zero `d`. Every caller
/// refuses a zero divisor first.
#[inline(always)]
fn mul_div_rem<R>(
    x: &U256,
    y: &U256,
    d: &U256,
    take: impl FnOnce(&Parts) -> R,
) -> R {
    let mut parts = Parts {
        lo: MaybeUninit::uninit(),
        hi: MaybeUninit::uninit(),
        rem: MaybeUninit::uninit(),
    };
    #[cfg(any(not(feature = "software-arithmetic"), target_arch = "riscv32"))]
    if thruput_xdr::syscalls::uninit_U256MulDivRem(
        bytes(x),
        bytes(y),
        bytes(d),
        bytes_mut(&mut parts.lo),
        bytes_mut(&mut parts.hi),
        bytes_mut(&mut parts.rem),
    )
    .is_err()
    {
        panic!("U256MulDivRem failed");
    }
    #[cfg(all(feature = "software-arithmetic", not(target_arch = "riscv32")))]
    {
        let (lo, hi) = x.widening_mul(*y);
        let (lo, hi, rem) = soft::div_rem(lo, hi, *d);
        parts.lo.write(lo);
        parts.hi.write(hi);
        parts.rem.write(rem);
    }
    take(&parts)
}

impl Parts {
    #[inline(always)]
    fn lo(&self) -> U256 {
        // SAFETY: `mul_div_rem` hands over only what the host wrote.
        unsafe { self.lo.assume_init() }
    }

    #[inline(always)]
    fn hi_is_zero(&self) -> bool {
        // SAFETY: as for `lo`.
        unsafe { self.hi.assume_init_ref() }.is_zero()
    }

    #[inline(always)]
    fn rem(&self) -> U256 {
        // SAFETY: as for `lo`.
        unsafe { self.rem.assume_init() }
    }

    #[inline(always)]
    fn rem_is_zero(&self) -> bool {
        // SAFETY: as for `lo`.
        unsafe { self.rem.assume_init_ref() }.is_zero()
    }
}

/// `x / d`. `d` is not zero.
#[inline(always)]
pub(crate) fn div(x: &U256, d: &U256) -> U256 {
    mul_div_rem(x, &U256::ONE, d, Parts::lo)
}

/// `x % d`. `d` is not zero.
#[inline(always)]
pub(crate) fn rem(x: &U256, d: &U256) -> U256 {
    mul_div_rem(x, &U256::ONE, d, Parts::rem)
}

/// `x / d` and `x % d`. `d` is not zero.
#[inline(always)]
pub(crate) fn div_rem(x: &U256, d: &U256) -> (U256, U256) {
    mul_div_rem(x, &U256::ONE, d, |parts| (parts.lo(), parts.rem()))
}

/// `floor(x * y / d)` and whether it was inexact, or `None` if the quotient
/// needs more than 256 bits. `d` is not zero.
#[inline(always)]
pub(crate) fn mul_div(x: &U256, y: &U256, d: &U256) -> Option<(U256, bool)> {
    mul_div_rem(x, y, d, |parts| {
        parts
            .hi_is_zero()
            .then(|| (parts.lo(), !parts.rem_is_zero()))
    })
}

/// `x * y % d`. `d` is not zero.
#[inline(always)]
pub(crate) fn mul_mod(x: &U256, y: &U256, d: &U256) -> U256 {
    mul_div_rem(x, y, d, Parts::rem)
}

/// `a * b` divided by `d`, by `U128MulDivRem`: what `take` reads from the
/// quotient's low and high 128 bits and the remainder.
///
/// # Panics
///
/// If the host operation fails, as for [`mul_div_rem`].
#[inline(always)]
fn mul_div_rem_u128<R>(
    a: &u128,
    b: &u128,
    d: &u128,
    take: impl FnOnce(u128, u128, u128) -> R,
) -> R {
    #[cfg(any(not(feature = "software-arithmetic"), target_arch = "riscv32"))]
    {
        let mut lo = MaybeUninit::uninit();
        let mut hi = MaybeUninit::uninit();
        let mut rem = MaybeUninit::uninit();
        if thruput_xdr::syscalls::uninit_U128MulDivRem(
            a, b, d, &mut lo, &mut hi, &mut rem,
        )
        .is_err()
        {
            panic!("U128MulDivRem failed");
        }
        // SAFETY: a successful call writes all three.
        unsafe { take(lo.assume_init(), hi.assume_init(), rem.assume_init()) }
    }
    #[cfg(all(feature = "software-arithmetic", not(target_arch = "riscv32")))]
    {
        let product = U256::widening_mul_u128(*a, *b);
        let (q, _, rem) =
            soft::div_rem(product, U256::ZERO, U256::from_u128(*d));
        let high = q.wrapping_shr(128);
        take(soft::low_u128(q), soft::low_u128(high), soft::low_u128(rem))
    }
}

/// `floor(sqrt(x))`, by `U256Sqrt`.
///
/// # Panics
///
/// If the host operation fails.
#[inline(always)]
pub(crate) fn sqrt(x: &U256) -> u128 {
    #[cfg(any(not(feature = "software-arithmetic"), target_arch = "riscv32"))]
    match thruput_xdr::syscalls::U256Sqrt(bytes(x)) {
        Ok(root) => root,
        Err(_) => panic!("U256Sqrt failed"),
    }
    #[cfg(all(feature = "software-arithmetic", not(target_arch = "riscv32")))]
    soft::sqrt(*x)
}

/// The same results in software, for a host program, which has no runtime to
/// ask. One bit per step, and nothing faster: only tools run it.
#[cfg(all(feature = "software-arithmetic", not(target_arch = "riscv32")))]
mod soft {
    use crate::U256;

    pub(super) fn low_u128(x: U256) -> u128 {
        let l = x.to_limbs();
        (l[0] as u128)
            | (l[1] as u128) << 32
            | (l[2] as u128) << 64
            | (l[3] as u128) << 96
    }

    /// The 512-bit `(hi:lo)` divided by `d`: the quotient's low and high 256
    /// bits, and the remainder. Schoolbook long division in base two.
    ///
    /// # Panics
    ///
    /// If `d` is zero, as the host operation fails.
    pub(super) fn div_rem(lo: U256, hi: U256, d: U256) -> (U256, U256, U256) {
        if d.is_zero() {
            panic!("U256MulDivRem failed");
        }
        let (mut q_lo, mut q_hi, mut rem) =
            (U256::ZERO, U256::ZERO, U256::ZERO);
        for i in (0..512_u32).rev() {
            let bit = if i < 256 { lo.bit(i) } else { hi.bit(i - 256) };
            // The bit shifted out of `rem` counts: `rem` was below `d`, so
            // the 257-bit value is below `2 * d` and one subtraction settles
            // it.
            let carry = rem.bit(255);
            rem = rem.wrapping_shl(1);
            if bit {
                rem |= U256::ONE;
            }
            if carry || rem.const_ge(d) {
                rem = rem.wrapping_sub(d);
                if i < 256 {
                    q_lo |= U256::ONE.wrapping_shl(i);
                } else {
                    q_hi |= U256::ONE.wrapping_shl(i - 256);
                }
            }
        }
        (q_lo, q_hi, rem)
    }

    /// The largest `r` with `r * r <= x`, one bit at a time from the top.
    pub(super) fn sqrt(x: U256) -> u128 {
        let mut root = 0_u128;
        for i in (0..128_u32).rev() {
            let candidate = root | 1 << i;
            if x.const_ge(U256::widening_mul_u128(candidate, candidate)) {
                root = candidate;
            }
        }
        root
    }
}

/// `floor(a * b / d)`, or `None` if it needs more than 128 bits. `d` is not
/// zero.
#[inline(always)]
pub(crate) fn mul_div_u128(a: &u128, b: &u128, d: &u128) -> Option<u128> {
    mul_div_rem_u128(a, b, d, |lo, hi, _| (hi == 0).then_some(lo))
}

/// `floor(a * b / d)` and whether it was inexact, or `None` if the quotient
/// needs more than 128 bits. `d` is not zero.
#[inline(always)]
pub(crate) fn mul_div_u128_inexact(
    a: &u128,
    b: &u128,
    d: &u128,
) -> Option<(u128, bool)> {
    mul_div_rem_u128(a, b, d, |lo, hi, rem| {
        (hi == 0).then_some((lo, rem != 0))
    })
}
