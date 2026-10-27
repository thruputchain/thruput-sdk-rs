//! Exhaustive shape coverage for the `unsafe` in `div`'s Knuth D.
//!
//! `knuth_d` keeps its two scratch buffers as `MaybeUninit` and initialises
//! only `vn[0..dn]` and `un[0..=nn]`, so soundness rests entirely on a claim
//! about indices: nothing reads above those bounds, for any `(nn, dn)` the
//! callers can produce. That is what this file tests, and it is why it exists
//! separately from `differential.rs`.
//!
//! The difference is deliberate. `differential.rs` samples: it throws tens of
//! thousands of random operands at an oracle, which is the right way to find
//! an arithmetic mistake but leaves shape coverage to chance. Here the shapes
//! *are* the subject, so every `(nn, dn)` pair both widths admit is visited by
//! construction, with the normalisation shift forced both ways -- `s == 0` and
//! `s != 0` take different index paths through the prologue.
//!
//! Being exhaustive over shapes rather than deep over values also makes this
//! the target Miri can afford: `.github/workflows/miri.yml` runs this file,
//! where the sampling suites would take hours under the interpreter.
//!
//! Correctness is checked by the division identity rather than an oracle --
//! `q * d + r == n` with `r < d` determines `(q, r)` uniquely, so it needs no
//! second implementation and stays cheap.

use thruput_u256::U256;

/// Builds a value with exactly `limbs` 32-bit limbs, top limb chosen so the
/// normalisation shift is zero (`top_bit_set`) or maximal (not).
fn with_limbs(limbs: usize, top_bit_set: bool) -> U256 {
    assert!((1..=8).contains(&limbs));
    let mut out = U256::ZERO;
    for i in 0..limbs {
        // A varied but deterministic body; the top limb is overwritten below.
        let limb = 0x9e37_79b9u32.wrapping_mul(i as u32 + 1) | 1;
        out |= U256::from_u32(limb).wrapping_shl(32 * i as u32);
    }
    let top = if top_bit_set { 0xffff_ffffu32 } else { 1u32 };
    // Clear the top limb, then set it.
    let mask = U256::from_u32(u32::MAX).wrapping_shl(32 * (limbs as u32 - 1));
    out &= !mask;
    out | U256::from_u32(top).wrapping_shl(32 * (limbs as u32 - 1))
}

/// `a <= b` on the `(low, high)` pairs `widening_mul` returns.
fn le512(a: (U256, U256), b: (U256, U256)) -> bool {
    let ((a_lo, a_hi), (b_lo, b_hi)) = (a, b);
    if a_hi != b_hi {
        a_hi < b_hi
    } else {
        a_lo <= b_lo
    }
}

/// `q * d + r == n`, `r < d`. Uniquely determines the pair, so it needs no
/// oracle.
#[track_caller]
fn check(n: U256, d: U256) {
    let (q, r) = n.div_rem(d);
    assert!(r < d, "remainder not reduced: {n:?} / {d:?}");
    let (prod, overflow) = q.overflowing_mul(d);
    assert!(!overflow, "q * d overflowed: {n:?} / {d:?}");
    let (back, carry) = prod.overflowing_add(r);
    assert!(!carry, "q * d + r overflowed: {n:?} / {d:?}");
    assert_eq!(back, n, "q * d + r != n for {n:?} / {d:?}");
}

/// Every `(nn, dn)` the narrow instantiation admits, with the normalisation
/// shift forced both ways on each side.
///
/// `dn == 1` never reaches Knuth D (it routes to `div_rem_small`), but it is
/// swept anyway so the dispatch boundary itself is covered.
#[test]
fn every_dividend_and_divisor_limb_count() {
    for nn in 1..=8 {
        for dn in 1..=nn {
            for n_top in [false, true] {
                for d_top in [false, true] {
                    check(with_limbs(nn, n_top), with_limbs(dn, d_top));
                }
            }
        }
    }
}

/// A divisor wider than the dividend takes the early-out rather than the
/// loop, and `nn - dn + 1` would underflow if it did not.
#[test]
fn divisor_wider_than_dividend() {
    for nn in 1..=7 {
        for dn in (nn + 1)..=8 {
            let n = with_limbs(nn, true);
            let d = with_limbs(dn, true);
            let (q, r) = n.div_rem(d);
            assert_eq!(q, U256::ZERO);
            assert_eq!(r, n);
        }
    }
}

/// The wide (512-bit dividend) instantiation, reached through `mul_div`.
///
/// `a * b` gives a genuine 16-limb dividend, so this sweeps `nn` well past
/// what the narrow path can produce while `dn` stays within a `U256`.
#[test]
fn wide_dividend_across_every_divisor_limb_count() {
    for a_limbs in 1..=8 {
        for b_limbs in 1..=8 {
            for dn in 1..=8 {
                for d_top in [false, true] {
                    let a = with_limbs(a_limbs, true);
                    let b = with_limbs(b_limbs, true);
                    let d = with_limbs(dn, d_top);
                    // Only the quotients that fit are defined; the rest
                    // exercise the same indices and report `None`.
                    if let Some(q) = U256::checked_mul_div(a, b, d) {
                        // `q * d <= a * b < (q + 1) * d`, all in 512 bits --
                        // `q * d` itself routinely exceeds 256, since it is
                        // bounded by the product rather than by a `U256`.
                        let ab = a.widening_mul(b);
                        assert!(le512(q.widening_mul(d), ab), "q * d > a * b");
                        let (next, wrapped) = q.overflowing_add(U256::ONE);
                        if !wrapped {
                            assert!(
                                !le512(next.widening_mul(d), ab),
                                "(q + 1) * d <= a * b, so q is too small"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Operands that force the add-back correction, which is the one path that
/// writes `un` a second time and reads `un[j + dn]` after doing so.
///
/// Reproduced from the estimate worst cases in `differential.rs`, but here as
/// fixed limb patterns so Miri visits them without a random sweep.
#[test]
fn add_back_correction_path() {
    for v1 in [0x8000_0000u32, 0x8000_0001, 0xc000_0001, u32::MAX] {
        for v0 in [1u32, 2, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
            for r1 in [0u32, 1, v0 / 2, v0.saturating_sub(1)] {
                let d = U256::from_limbs([v0, v1, 0, 0, 0, 0, 0, 0]);
                if d.is_zero() {
                    continue;
                }
                for r0 in [0u32, 1, 0x8000_0000, u32::MAX] {
                    let n = U256::from_limbs([r0, r1, v1, 0, 0, 0, 0, 0]);
                    check(n, d);

                    // Three-limb divisor: the same shape one limb wider.
                    let d3 = U256::from_limbs([r0, v0, v1, 0, 0, 0, 0, 0]);
                    if !d3.is_zero() {
                        let n4 = U256::from_limbs([
                            r0,
                            r1,
                            v0.wrapping_sub(1),
                            v1,
                            0,
                            0,
                            0,
                            0,
                        ]);
                        check(n4, d3);
                    }
                }
            }
        }
    }
}

/// `s == 0` (divisor top bit already set) and `s == 31` (top limb is 1) are
/// the two extremes of the normalisation prologue, where `un[nn]` is written
/// from a shift that is either skipped or maximal.
#[test]
fn normalisation_shift_extremes() {
    for nn in 2..=8 {
        for dn in 2..=nn {
            let n = with_limbs(nn, true);
            check(n, with_limbs(dn, true)); // s == 0
            check(n, with_limbs(dn, false)); // s == 31
        }
    }
}
