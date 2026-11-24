//! The runtime's arithmetic host operations, through their generated
//! bindings. Every division on these types is one of these calls: an `ecall`
//! in a contract, the native executor in a native test.
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
#[inline(always)]
pub(crate) fn bytes(value: &U256) -> &[u8; 32] {
    // SAFETY: `U256` is a transparent `[u32; 8]`: 32 bytes, aligned at least
    // as strictly as `[u8; 32]`.
    unsafe { &*core::ptr::from_ref(value).cast() }
}

/// `value`, as the 32 bytes the runtime writes.
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
