//! 64-bit and 128-bit integer division for the hogd rv32 VM.
//!
//! RV32IM divides 32-bit values. For anything wider the compiler emits a
//! call: `__udivdi3` and its siblings for 64 bits, `__udivti3` and its
//! siblings for 128. compiler_builtins' versions fall back to binary long
//! division, one quotient bit per iteration, whenever the divisor is too
//! wide for a native divide. That costs hundreds of instructions for a
//! 64-bit quotient and thousands for a 128-bit one.
//!
//! These replace them. A 64-bit division is schoolbook division on 32-bit
//! limbs: each quotient limb comes from two native divides and a correction
//! (Knuth's algorithm D). A divisor below `2^16` needs no estimate at all:
//! two native divides per limb give exact digits, at 64 and at 128 bits.
//!
//! Any other 128-bit division goes to the runtime's `U128MulDivRem`, with a
//! multiplier of one. Host tests stand in for it with native 128-bit division.
//!
//! LLVM calls the 128-bit routines only under
//! `-C llvm-args=-expand-div-rem-bits=128`, which the contract rustflags
//! set. Without it LLVM expands every 128-bit `/` and `%` inline as a
//! bit-serial loop: about 2 KiB of code per division, and several thousand
//! gas each time it runs.
//!
//! These are strong definitions overriding compiler_builtins' weak symbols
//! at link time, like the routines in `mem.rs`. CI asserts the override
//! held per contract build (gnaw's `elf::check_div_symbols`).
//!
//! RECURSION GUARD: nothing here may divide a value wider than 32 bits. A
//! 64-bit `/` or `%` would compile to a call back into this module. The
//! `int_division` fixture runs every routine as rv32, so such a call would
//! exhaust its gas.
//!
//! Each routine returns from a short entry when a native divide settles
//! the result, in registers the caller already saved. The harder cases
//! tail-call a body with its own frame. A register saved and restored
//! costs four gas in the test executor, so the split keeps that cost off
//! the common path. The `int_division` fixture's rv32 test bounds the gas
//! per division.

/// The base of a half limb.
const HB: u32 = 1 << 16;

/// Flag: return the remainder instead of the quotient.
pub(crate) const REM: u32 = 1;
/// Flag: the operands are two's complement.
pub(crate) const SIGNED: u32 = 2;

/// `x >> (32 - s)` for `s` in `0..32`: the bits a left shift by `s` pushes
/// out of `x`, and zero at `s == 0`.
#[inline(always)]
const fn spill(x: u32, s: u32) -> u32 {
    (x >> 1) >> (31 - s)
}

/// Divides `(hi:lo)` by `d`. Requires `hi < d` and the top bit of `d` set.
#[inline(always)]
const fn div_2by1(hi: u32, lo: u32, d: u32) -> (u32, u32) {
    let (d1, d0) = (d >> 16, d & 0xffff);
    let (l1, l0) = (lo >> 16, lo & 0xffff);

    let mut q1 = hi / d1;
    let mut rhat = hi - q1 * d1;
    while q1 >= HB || q1 * d0 > (rhat << 16) | l1 {
        q1 -= 1;
        rhat += d1;
        if rhat >= HB {
            break;
        }
    }
    let mid = ((hi << 16) | l1).wrapping_sub(q1.wrapping_mul(d));

    let mut q0 = mid / d1;
    rhat = mid - q0 * d1;
    while q0 >= HB || q0 * d0 > (rhat << 16) | l0 {
        q0 -= 1;
        rhat += d1;
        if rhat >= HB {
            break;
        }
    }
    let rem = ((mid << 16) | l0).wrapping_sub(q0.wrapping_mul(d));
    ((q1 << 16) | q0, rem)
}

/// Divides `(hi:lo)` by `d`. Requires `hi < d < 2^16`.
#[inline(always)]
const fn div_2by1_small(hi: u32, lo: u32, d: u32) -> (u32, u32) {
    let t = (hi << 16) | (lo >> 16);
    let q1 = t / d;
    let t = ((t % d) << 16) | (lo & 0xffff);
    ((q1 << 16) | (t / d), t % d)
}

/// Picks the quotient or the remainder, as `flags` asks.
#[inline(always)]
const fn pick(q: u64, r: u64, flags: u32) -> u64 {
    if flags & REM != 0 { r } else { q }
}

/// The 64-bit libcalls' shared entry: `n / d`, or `n % d` under [`REM`].
/// `d` must not be zero.
///
/// The cases a native divide settles return from here, in registers the
/// caller already saved. The rest tail-call a body with its own frame.
#[cfg_attr(target_arch = "riscv32", inline(never))]
pub(crate) extern "C" fn divrem64(n: u64, d: u64, flags: u32) -> u64 {
    let (n1, n0) = ((n >> 32) as u32, n as u32);
    let (d1, d0) = ((d >> 32) as u32, d as u32);
    if flags & SIGNED != 0 && ((n1 | d1) as i32) < 0 {
        return negative64(n, d, flags);
    }
    if d1 != 0 {
        if n1 < d1 {
            return pick(0, n, flags);
        }
    } else if n1 == 0 {
        return if flags & REM != 0 {
            (n0 % d0) as u64
        } else {
            (n0 / d0) as u64
        };
    } else if d0 < HB {
        // The high limb divides natively. Its remainder is below the
        // divisor, so the low limb's quotient fits a limb.
        let (q0, r) = div_2by1_small(n1 % d0, n0, d0);
        return pick((((n1 / d0) as u64) << 32) | q0 as u64, r as u64, flags);
    }
    estimate64(n, d, flags)
}

/// Signed division with a negative operand: divides the magnitudes.
#[cfg_attr(target_arch = "riscv32", inline(never))]
extern "C" fn negative64(n: u64, d: u64, flags: u32) -> u64 {
    let (neg_n, neg_d) = ((n as i64) < 0, (d as i64) < 0);
    let value = divrem64(
        if neg_n { n.wrapping_neg() } else { n },
        if neg_d { d.wrapping_neg() } else { d },
        flags & REM,
    );
    let negate = if flags & REM != 0 {
        neg_n
    } else {
        neg_n != neg_d
    };
    if negate { value.wrapping_neg() } else { value }
}

/// Unsigned 64-bit division that needs a quotient estimate: a divisor of
/// two limbs, or of one limb at or above `2^16` under a two-limb numerator.
#[cfg_attr(target_arch = "riscv32", inline(never))]
extern "C" fn estimate64(n: u64, d: u64, flags: u32) -> u64 {
    let (n1, n0) = ((n >> 32) as u32, n as u32);
    let (d1, d0) = ((d >> 32) as u32, d as u32);
    // Both cases are one shape: a three-limb numerator `(a2:a1:a0)` over a
    // two-limb divisor `(b1:b0)` with `b1 != 0`, and a one-limb quotient. A
    // one-limb divisor reaches it shifted up a limb, after its high limb
    // has divided natively.
    let (q1, a2, a1, a0, b1, b0) = if d1 == 0 {
        (n1 / d0, n1 % d0, n0, 0, d0, 0)
    } else {
        (0, 0, n1, n0, d1, d0)
    };
    let s = b1.leading_zeros();
    let (mut q, _) = div_2by1(
        (a2 << s) | spill(a1, s),
        (a1 << s) | spill(a0, s),
        (b1 << s) | spill(b0, s),
    );
    // The estimate is at most two too large. Compare `q * b`, 96 bits,
    // with `a`.
    let lo = (q as u64) * (b0 as u64);
    let mut p_hi = (q as u64) * (b1 as u64) + (lo >> 32);
    let mut p_lo = lo as u32;
    let a_hi = ((a2 as u64) << 32) | a1 as u64;
    while p_hi > a_hi || (p_hi == a_hi && p_lo > a0) {
        q -= 1;
        p_hi = p_hi.wrapping_sub(b1 as u64 + (p_lo < b0) as u64);
        p_lo = p_lo.wrapping_sub(b0);
    }
    if flags & REM == 0 {
        return ((q1 as u64) << 32) | q as u64;
    }
    // The remainder is `a - q * b`: two limbs, in the numerator's frame.
    let r0 = a0.wrapping_sub(p_lo);
    let r1 = a1
        .wrapping_sub(p_hi as u32)
        .wrapping_sub((a0 < p_lo) as u32);
    if d1 == 0 {
        r1 as u64
    } else {
        ((r1 as u64) << 32) | r0 as u64
    }
}

/// A 128-bit value, least significant limb first.
pub(crate) type Wide = [u32; 4];

/// Two's complement negation.
#[inline(always)]
const fn neg(x: Wide) -> Wide {
    let mut out = [0; 4];
    let mut carry = true;
    let mut i = 0;
    while i < 4 {
        (out[i], carry) = (!x[i]).overflowing_add(carry as u32);
        i += 1;
    }
    out
}

/// The 128-bit libcalls' shared entry: writes `n / d`, or `n % d` under
/// [`REM`], to `out`.
#[cfg_attr(target_arch = "riscv32", inline(never))]
pub(crate) unsafe extern "C" fn divrem128(
    out: *mut Wide,
    n: *const Wide,
    d: *const Wide,
    flags: u32,
) {
    unsafe {
        let (n3, d3) = ((*n)[3], (*d)[3]);
        let d2 = (*d)[2];
        if flags & SIGNED != 0 && ((n3 | d3) as i32) < 0 {
            return negative128(out, n, d, flags);
        }
        if d2 | d3 | (*d)[1] == 0 && (*d)[0] < HB {
            return small128(out, n, d, flags);
        }
        host128(out, n, d, flags)
    }
}

/// `x` as one 128-bit integer.
#[inline(always)]
const fn join(x: Wide) -> u128 {
    x[0] as u128
        | (x[1] as u128) << 32
        | (x[2] as u128) << 64
        | (x[3] as u128) << 96
}

/// `x` as limbs.
#[inline(always)]
const fn split(x: u128) -> Wide {
    [
        x as u32,
        (x >> 32) as u32,
        (x >> 64) as u32,
        (x >> 96) as u32,
    ]
}

/// Unsigned division by the runtime: `U128MulDivRem` with a multiplier of
/// one. `d` is not zero: the language checks before it divides.
#[cfg(target_arch = "riscv32")]
#[inline(never)]
unsafe extern "C" fn host128(
    out: *mut Wide,
    n: *const Wide,
    d: *const Wide,
    flags: u32,
) {
    let (n, d) = unsafe { (join(*n), join(*d)) };
    let (quotient, _, remainder) =
        match thruput_xdr::syscalls::U128MulDivRem(&n, &1, &d) {
            Ok(parts) => parts,
            Err(_) => panic!("U128MulDivRem failed"),
        };
    let value = if flags & REM != 0 {
        remainder
    } else {
        quotient
    };
    unsafe { *out = split(value) };
}

/// What the runtime answers, for the host tests.
#[cfg(not(target_arch = "riscv32"))]
unsafe extern "C" fn host128(
    out: *mut Wide,
    n: *const Wide,
    d: *const Wide,
    flags: u32,
) {
    let (n, d) = unsafe { (join(*n), join(*d)) };
    let value = if flags & REM != 0 { n % d } else { n / d };
    unsafe { *out = split(value) };
}

/// Signed division with a negative operand: divides the magnitudes.
#[cfg_attr(target_arch = "riscv32", inline(never))]
unsafe extern "C" fn negative128(
    out: *mut Wide,
    n: *const Wide,
    d: *const Wide,
    flags: u32,
) {
    unsafe {
        let (mut a, mut b) = (*n, *d);
        let (neg_n, neg_d) = ((a[3] as i32) < 0, (b[3] as i32) < 0);
        if neg_n {
            a = neg(a);
        }
        if neg_d {
            b = neg(b);
        }
        let mut value = [0; 4];
        divrem128(&mut value, &a, &b, flags & REM);
        let negate = if flags & REM != 0 {
            neg_n
        } else {
            neg_n != neg_d
        };
        *out = if negate { neg(value) } else { value };
    }
}

/// Unsigned division by a divisor below `2^16`: two native half-limb steps
/// per limb.
#[cfg_attr(target_arch = "riscv32", inline(never))]
unsafe extern "C" fn small128(
    out: *mut Wide,
    n: *const Wide,
    d: *const Wide,
    flags: u32,
) {
    unsafe {
        let (n, d0) = (*n, (*d)[0]);
        let (q3, r) = div_2by1_small(0, n[3], d0);
        let (q2, r) = div_2by1_small(r, n[2], d0);
        let (q1, r) = div_2by1_small(r, n[1], d0);
        let (q0, r) = div_2by1_small(r, n[0], d0);
        *out = if flags & REM != 0 {
            [r, 0, 0, 0]
        } else {
            [q0, q1, q2, q3]
        };
    }
}

// The exported strong symbols (rv32 contracts only). Each loads its flags
// and tail-calls the shared entry: two instructions.

macro_rules! libcalls64 {
    ($($name:ident($ty:ty) => $flags:expr;)*) => {$(
        #[cfg(target_arch = "riscv32")]
        #[unsafe(no_mangle)]
        extern "C" fn $name(n: $ty, d: $ty) -> $ty {
            divrem64(n as u64, d as u64, $flags) as $ty
        }
    )*};
}

libcalls64! {
    __udivdi3(u64) => 0;
    __umoddi3(u64) => REM;
    __divdi3(i64) => SIGNED;
    __moddi3(i64) => SIGNED | REM;
}

// On rv32 the C ABI passes a 128-bit integer by reference and returns it
// through a pointer in the first argument register. LLVM calls
// `__udivti3(u128, u128) -> u128` that way, so these take the three
// pointers. Spelling the signature with `u128` instead makes rustc copy
// both operands and the result through the stack around the tail call.
macro_rules! libcalls128 {
    ($($name:ident => $flags:expr;)*) => {$(
        #[cfg(target_arch = "riscv32")]
        #[unsafe(no_mangle)]
        unsafe extern "C" fn $name(
            out: *mut Wide,
            n: *const Wide,
            d: *const Wide,
        ) {
            unsafe { divrem128(out, n, d, $flags) }
        }
    )*};
}

libcalls128! {
    __udivti3 => 0;
    __umodti3 => REM;
    __divti3 => SIGNED;
    __modti3 => SIGNED | REM;
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [u32; 4] = [0, REM, SIGNED, SIGNED | REM];

    /// What the host computes, or `None` where the language itself
    /// refuses: a zero divisor, or the signed quotient that overflows.
    /// Compiled code never passes those operands to a libcall.
    fn native(n: u64, d: u64, flags: u32) -> Option<u64> {
        match (flags & SIGNED != 0, flags & REM != 0) {
            (false, false) => n.checked_div(d),
            (false, true) => n.checked_rem(d),
            (true, false) => {
                (n as i64).checked_div(d as i64).map(|q| q as u64)
            }
            (true, true) => (n as i64).checked_rem(d as i64).map(|r| r as u64),
        }
    }

    fn check(n: u64, d: u64) {
        for flags in KINDS {
            if let Some(expected) = native(n, d, flags) {
                assert_eq!(
                    divrem64(n, d, flags),
                    expected,
                    "n={n:#x} d={d:#x} flags={flags}"
                );
            }
        }
    }

    /// Limb values at the boundaries the routines branch on.
    const LIMBS: [u32; 8] = [
        0,
        1,
        0xFFFF,
        0x0001_0000,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
    ];

    /// xorshift64: deterministic operands without a dependency.
    struct Rng(u64);

    impl Rng {
        /// A value of exactly `bits` significant bits.
        fn sized(&mut self, bits: u32) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 | 1 << 63) >> (64 - bits)
        }
    }

    #[test]
    fn matches_native_at_limb_boundaries() {
        for n1 in LIMBS {
            for n0 in LIMBS {
                for d1 in LIMBS {
                    for d0 in LIMBS {
                        check(
                            (n1 as u64) << 32 | n0 as u64,
                            (d1 as u64) << 32 | d0 as u64,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn takes_the_estimate_correction() {
        // Estimates one and two too large.
        check(0x1_0000_0000, 0x1_0000_0001);
        check(0x2_0000_0001, 0x1_0000_0001);
        check(0x8000_0000_0000_0000, 0x8000_0000_FFFF_FFFF);
        check(0x8000_0000_0000_0000, 0x8000_0000_0000_0001);
        check(0xFFFF_FFFE_FFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF);
        // Signed operands at the extremes.
        check(1 << 63, 1);
        check(1 << 63, 1 << 63);
        check(u64::MAX, 1 << 63);
    }

    // Too slow under Miri: the two tests above cover the same code there.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn matches_native_at_every_width() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for n_bits in 1..=64 {
            for d_bits in 1..=64 {
                for _ in 0..16 {
                    let (n, d) = (rng.sized(n_bits), rng.sized(d_bits));
                    check(n, d);
                    // Both signs, by magnitude.
                    check(n.wrapping_neg(), d);
                    check(n, d.wrapping_neg());
                }
            }
        }
    }

    // The 128-bit routines.

    /// [`native`] for 128-bit operands.
    fn native128(n: u128, d: u128, flags: u32) -> Option<u128> {
        match (flags & SIGNED != 0, flags & REM != 0) {
            (false, false) => n.checked_div(d),
            (false, true) => n.checked_rem(d),
            (true, false) => {
                (n as i128).checked_div(d as i128).map(|q| q as u128)
            }
            (true, true) => {
                (n as i128).checked_rem(d as i128).map(|r| r as u128)
            }
        }
    }

    fn check128(n: u128, d: u128) {
        for flags in KINDS {
            if let Some(expected) = native128(n, d, flags) {
                let mut out = [0; 4];
                unsafe { divrem128(&mut out, &split(n), &split(d), flags) };
                assert_eq!(
                    join(out),
                    expected,
                    "n={n:#x} d={d:#x} flags={flags}"
                );
            }
        }
    }

    /// Six of [`LIMBS`]: pairing eight limbs takes 1.7 million divisions
    /// with six values each, and ten times that with eight.
    const WIDE_LIMBS: [u32; 6] =
        [0, 1, 0x0001_0000, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF];

    impl Rng {
        /// A 128-bit value of exactly `bits` significant bits.
        fn wide(&mut self, bits: u32) -> u128 {
            let x = (self.sized(64) as u128) << 64 | self.sized(64) as u128;
            x >> (128 - bits)
        }
    }

    // Too slow under Miri: `div128_matches_native_at_sampled_widths`
    // covers the same code there.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn div128_matches_native_at_limb_boundaries() {
        let mut index = [0usize; 8];
        loop {
            let limb = |i: usize| WIDE_LIMBS[index[i]];
            check128(
                join([limb(0), limb(1), limb(2), limb(3)]),
                join([limb(4), limb(5), limb(6), limb(7)]),
            );
            let mut i = 0;
            while i < 8 {
                index[i] += 1;
                if index[i] < WIDE_LIMBS.len() {
                    break;
                }
                index[i] = 0;
                i += 1;
            }
            if i == 8 {
                break;
            }
        }
    }

    #[test]
    fn div128_signed_extremes() {
        check128(1 << 127, 1);
        check128(1 << 127, 1 << 127);
        check128(u128::MAX, 1 << 127);
        check128((1 << 127) + 1, u128::MAX);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn div128_matches_native_at_every_width() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for n_bits in 1..=128 {
            for d_bits in 1..=128 {
                for _ in 0..4 {
                    let (n, d) = (rng.wide(n_bits), rng.wide(d_bits));
                    check128(n, d);
                    // Both signs, by magnitude.
                    check128(n.wrapping_neg(), d);
                    check128(n, d.wrapping_neg());
                }
            }
        }
    }

    #[test]
    fn div128_matches_native_at_sampled_widths() {
        // The width sweep above, thinned for Miri.
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for n_bits in [1, 31, 33, 64, 65, 97, 128] {
            for d_bits in [1, 16, 17, 32, 33, 64, 65, 96, 97, 128] {
                let (n, d) = (rng.wide(n_bits), rng.wide(d_bits));
                check128(n, d);
                check128(n.wrapping_neg(), d.wrapping_neg());
            }
        }
    }
}
