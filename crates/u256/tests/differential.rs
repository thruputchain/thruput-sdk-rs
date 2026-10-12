#![feature(cfg_overflow_checks)]
// `uint::construct_uint!` trips this in its own expansion; the swap suite
// carries the same allow where it invokes the macro.
#![allow(clippy::manual_div_ceil)]

//! Differential tests against Parity's `uint`, the implementation this crate
//! replaces. It is correct, just badly matched to 32-bit targets, which makes
//! it a good oracle: every operation here must agree with it exactly.
//!
//! The sqrt and division cases are ported from the swap suite's `swap-u256`,
//! including the random sweeps sized to hit Knuth D's add-back correction
//! (~2/2^32, so it needs volume).

use thruput_u256::{U256, checked_mul_div_u128, mul_div_u128};

uint::construct_uint! {
    /// The oracle: four 64-bit limbs.
    pub struct O(4);
}

fn to_oracle(n: U256) -> O {
    let l = n.to_limbs();
    O([
        (l[0] as u64) | ((l[1] as u64) << 32),
        (l[2] as u64) | ((l[3] as u64) << 32),
        (l[4] as u64) | ((l[5] as u64) << 32),
        (l[6] as u64) | ((l[7] as u64) << 32),
    ])
}

fn from_oracle(o: O) -> U256 {
    let w = o.0;
    U256::from_limbs([
        w[0] as u32,
        (w[0] >> 32) as u32,
        w[1] as u32,
        (w[1] >> 32) as u32,
        w[2] as u32,
        (w[2] >> 32) as u32,
        w[3] as u32,
        (w[3] >> 32) as u32,
    ])
}

/// Deterministic xorshift, so failures reproduce exactly.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_u128(&mut self) -> u128 {
        ((self.next_u64() as u128) << 64) | (self.next_u64() as u128)
    }

    /// A value with `limbs` significant 64-bit limbs.
    fn next_u256(&mut self, limbs: usize) -> U256 {
        let mut v = U256::ZERO;
        for _ in 0..limbs {
            v = v.wrapping_shl(64) | U256::from_u64(self.next_u64());
        }
        v
    }
}

/// Interesting values: zero, one, every limb boundary, and the extremes.
fn boundary_values() -> Vec<U256> {
    let mut v = vec![
        U256::ZERO,
        U256::ONE,
        U256::MAX,
        U256::MAX - U256::ONE,
        U256::from_u32(u32::MAX),
        U256::from_u64(u32::MAX as u64 + 1),
        U256::from_u64(u64::MAX),
        U256::from(u64::MAX as u128 + 1),
        U256::from(u128::MAX),
    ];
    for shift in 0..256u32 {
        let x = U256::ONE.wrapping_shl(shift);
        v.push(x);
        v.push(x.wrapping_sub(U256::ONE));
    }
    v
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

#[test]
fn oracle_conversion_round_trips() {
    let mut rng = Rng(0x1234_5678_9abc_def0);
    for _ in 0..2000 {
        let n = rng.next_u256(4);
        assert_eq!(from_oracle(to_oracle(n)), n);
    }
    for n in boundary_values() {
        assert_eq!(from_oracle(to_oracle(n)), n);
    }
}

#[test]
fn from_primitives_matches_oracle() {
    let mut rng = Rng(0xfeed_face_dead_beef);
    for _ in 0..2000 {
        let a = rng.next_u128();
        assert_eq!(to_oracle(U256::from(a)), O::from(a));
        // Through `Into`, which is the only route to the narrower `From`
        // impls now that the inherent `U256::from` shadows them.
        assert_eq!(to_oracle((a as u64).into()), O::from(a as u64));
        assert_eq!(to_oracle((a as u32).into()), O::from(a as u32));
        assert_eq!(to_oracle((a as u8).into()), O::from(a as u8));
    }
}

#[test]
fn try_into_u128_matches_oracle() {
    let mut rng = Rng(0x0bad_c0de_0bad_c0de);
    for limbs in 1..=4 {
        for _ in 0..2000 {
            let n = rng.next_u256(limbs);
            let o = to_oracle(n);
            match u128::try_from(n) {
                Ok(v) => {
                    assert!(limbs <= 2, "{n:?} fit u128 with {limbs} limbs");
                    assert_eq!(v, o.as_u128());
                }
                Err(_) => assert!(o > O::from(u128::MAX)),
            }
        }
    }
    assert_eq!(u128::try_from(U256::from(u128::MAX)).unwrap(), u128::MAX);
    assert!(u128::try_from(U256::from(u128::MAX) + U256::ONE).is_err());
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

#[test]
fn ordering_matches_oracle() {
    let vals = boundary_values();
    for a in &vals {
        for b in &vals {
            assert_eq!(
                a.cmp(b),
                to_oracle(*a).cmp(&to_oracle(*b)),
                "cmp({a:?}, {b:?})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Addition and subtraction
// ---------------------------------------------------------------------------

#[test]
fn add_sub_match_oracle_on_boundaries() {
    let vals = boundary_values();
    for a in &vals {
        for b in &vals {
            let (v, o) = a.overflowing_add(*b);
            let (ev, eo) = to_oracle(*a).overflowing_add(to_oracle(*b));
            assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} + {b:?}");

            let (v, o) = a.overflowing_sub(*b);
            let (ev, eo) = to_oracle(*a).overflowing_sub(to_oracle(*b));
            assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} - {b:?}");
        }
    }
}

#[test]
fn add_sub_match_oracle_on_random_inputs() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for a_limbs in 1..=4 {
        for b_limbs in 1..=4 {
            for _ in 0..1000 {
                let a = rng.next_u256(a_limbs);
                let b = rng.next_u256(b_limbs);

                let (v, o) = a.overflowing_add(b);
                let (ev, eo) = to_oracle(a).overflowing_add(to_oracle(b));
                assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} + {b:?}");

                let (v, o) = a.overflowing_sub(b);
                let (ev, eo) = to_oracle(a).overflowing_sub(to_oracle(b));
                assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} - {b:?}");
            }
        }
    }
}

#[test]
fn saturating_and_checked_add_sub() {
    assert_eq!(U256::MAX.saturating_add(U256::ONE), U256::MAX);
    assert_eq!(U256::ZERO.saturating_sub(U256::ONE), U256::ZERO);
    assert_eq!(U256::MAX.checked_add(U256::ONE), None);
    assert_eq!(U256::ZERO.checked_sub(U256::ONE), None);
    assert_eq!(U256::ONE.checked_add(U256::ONE), Some(U256::from(2)));
    assert_eq!(U256::ONE.checked_sub(U256::ONE), Some(U256::ZERO));
}

// ---------------------------------------------------------------------------
// Multiplication
// ---------------------------------------------------------------------------

#[test]
fn mul_matches_oracle_on_boundaries() {
    let vals = boundary_values();
    for a in &vals {
        for b in &vals {
            let (v, o) = a.overflowing_mul(*b);
            let (ev, eo) = to_oracle(*a).overflowing_mul(to_oracle(*b));
            assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} * {b:?}");
        }
    }
}

#[test]
fn mul_matches_oracle_on_random_inputs() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for a_limbs in 1..=4 {
        for b_limbs in 1..=4 {
            for _ in 0..1000 {
                let a = rng.next_u256(a_limbs);
                let b = rng.next_u256(b_limbs);
                let (v, o) = a.overflowing_mul(b);
                let (ev, eo) = to_oracle(a).overflowing_mul(to_oracle(b));
                assert_eq!((to_oracle(v), o), (ev, eo), "{a:?} * {b:?}");
            }
        }
    }
}

#[test]
fn widening_mul_is_exact() {
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    let mut cases: Vec<(u128, u128)> = vec![
        (0, 0),
        (1, 1),
        (3, 7),
        (u128::MAX, 0),
        (u128::MAX, 1),
        (u128::MAX, u128::MAX),
        (i128::MAX as u128, i128::MAX as u128),
        (1 << 127, 1 << 127),
    ];
    for _ in 0..5000 {
        cases.push((rng.next_u128(), rng.next_u128()));
    }

    for (a, b) in cases {
        // The oracle multiplies in 256 bits, where a u128 x u128 product
        // always fits, so this is the exact expected value.
        let expected = O::from(a) * O::from(b);
        assert_eq!(to_oracle(U256::widening_mul(a, b)), expected, "{a} * {b}");
    }
}

#[test]
fn widening_mul_agrees_with_general_mul() {
    let mut rng = Rng(0x5ee_d5ee_d5ee_d5ee);
    for _ in 0..5000 {
        let a = rng.next_u128();
        let b = rng.next_u128();
        let (general, overflow) = U256::from(a).overflowing_mul(U256::from(b));
        assert!(!overflow, "u128 x u128 must fit 256 bits: {a} * {b}");
        assert_eq!(U256::widening_mul(a, b), general, "{a} * {b}");
    }
}

#[test]
fn mul_u32_matches_general_mul() {
    let mut rng = Rng(0xabcd_ef01_2345_6789);
    for limbs in 1..=4 {
        for _ in 0..2000 {
            let a = rng.next_u256(limbs);
            let b = rng.next_u64() as u32;
            let (v, o) = a.overflowing_mul_u32(b);
            let (ev, eo) = a.overflowing_mul(U256::from_u32(b));
            assert_eq!((v, o), (ev, eo), "{a:?} * {b}");
        }
    }
    // Scaling constants the swap suite actually uses.
    for b in [3u32, 5, 997, 1000] {
        let a = U256::from(u128::MAX);
        assert_eq!(
            to_oracle(a.overflowing_mul_u32(b).0),
            to_oracle(a).overflowing_mul(O::from(b)).0
        );
    }
}

// ---------------------------------------------------------------------------
// Shifts and bitwise
// ---------------------------------------------------------------------------

#[test]
fn shifts_match_oracle() {
    let mut rng = Rng(0x1111_2222_3333_4444);
    let mut vals = boundary_values();
    for _ in 0..200 {
        vals.push(rng.next_u256(4));
    }

    for n in &vals {
        for shift in 0..256u32 {
            assert_eq!(
                to_oracle(n.wrapping_shl(shift)),
                to_oracle(*n) << (shift as usize),
                "{n:?} << {shift}"
            );
            assert_eq!(
                to_oracle(n.wrapping_shr(shift)),
                to_oracle(*n) >> (shift as usize),
                "{n:?} >> {shift}"
            );
        }
    }
}

#[test]
fn shift_edges() {
    assert_eq!(U256::ONE.checked_shl(256), None);
    assert_eq!(U256::ONE.checked_shr(256), None);
    assert_eq!(U256::ONE.overflowing_shl(256), (U256::ONE, true));
    assert_eq!(U256::MAX.wrapping_shl(255), U256::ONE.wrapping_shl(255));
    assert_eq!(U256::MAX.wrapping_shr(255), U256::ONE);
    assert_eq!(U256::MAX.wrapping_shr(256), U256::MAX); // masked to 0
}

#[test]
fn bitwise_matches_oracle() {
    let mut rng = Rng(0x7777_8888_9999_aaaa);
    for _ in 0..2000 {
        let a = rng.next_u256(4);
        let b = rng.next_u256(4);
        assert_eq!(to_oracle(a & b), to_oracle(a) & to_oracle(b));
        assert_eq!(to_oracle(a | b), to_oracle(a) | to_oracle(b));
        assert_eq!(to_oracle(a ^ b), to_oracle(a) ^ to_oracle(b));
        assert_eq!(to_oracle(!a), !to_oracle(a));
    }
}

#[test]
fn bit_len_matches_oracle() {
    let mut vals = boundary_values();
    let mut rng = Rng(0xbbbb_cccc_dddd_eeee);
    for _ in 0..500 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        assert_eq!(n.bit_len() as usize, to_oracle(n).bits(), "{n:?}");
        assert_eq!(n.leading_zeros(), 256 - n.bit_len());
    }
}

// ---------------------------------------------------------------------------
// Division
// ---------------------------------------------------------------------------

fn check_div(n: U256, d: U256) {
    let (q, r) = n.div_rem(d);
    let (eq, er) = (to_oracle(n) / to_oracle(d), to_oracle(n) % to_oracle(d));
    assert_eq!((to_oracle(q), to_oracle(r)), (eq, er), "{n:?} / {d:?}");
}

#[test]
fn div_matches_oracle_on_small_values() {
    for a in 0u64..64 {
        for b in 1u64..64 {
            check_div(U256::from_u64(a), U256::from_u64(b));
        }
    }
}

#[test]
fn div_matches_oracle_at_boundaries() {
    let vals = boundary_values();
    for n in &vals {
        for d in &vals {
            if !d.is_zero() {
                check_div(*n, *d);
            }
        }
    }
}

// Knuth D estimates each quotient limb and corrects when the estimate is too
// big. The correction fires with probability ~2/2^32, so it needs volume to be
// exercised at all -- hence a wide sweep across every combination of dividend
// and divisor limb counts.
#[test]
fn div_matches_oracle_on_random_inputs() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for n_limbs in 1..=4 {
        for d_limbs in 1..=4 {
            for _ in 0..2000 {
                let n = rng.next_u256(n_limbs);
                let d = rng.next_u256(d_limbs);
                if d.is_zero() {
                    continue;
                }
                check_div(n, d);
            }
        }
    }
}

// Divisors just under a limb boundary maximise the chance of a bad quotient
// estimate, which is what drives the add-back path.
#[test]
fn div_matches_oracle_on_estimate_worst_cases() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..4000 {
        let top = U256::from_u32(u32::MAX);
        for shift in [32u32, 64, 96, 128] {
            let d = top.wrapping_shl(shift)
                | U256::from_u64(rng.next_u64() & 0xffff);
            if d.is_zero() {
                continue;
            }
            let n = rng.next_u256(4);
            check_div(n, d);
        }
    }
}

// The quotient-digit estimate takes a separate branch when the running
// remainder's top limb equals the divisor's: the true digit is then at least
// `B`, so the estimate is clamped instead of divided out. Random inputs reach
// that branch with probability ~2^-32 per digit, so it needs building by hand.
//
// With `v` already normalised (top bit set, so the division does not shift),
// `u = q * v * B^k + R` divides as digit `q` followed by the digits of
// `R / v`. Give `R` the same leading limb as `v` -- legal as long as
// `R < v * B^k`, which the lower limbs arrange -- and the next digit's
// estimate starts on the clamp.
fn check_div_from_limbs(v: &[u32], r: &[u32], q: u32) {
    let shift = 32 * (r.len() - v.len()) as u32;

    let mut limbs = [0u32; 8];
    limbs[..v.len()].copy_from_slice(v);
    let v = U256::from_limbs(limbs);

    let mut limbs = [0u32; 8];
    limbs[..r.len()].copy_from_slice(r);
    let r = U256::from_limbs(limbs);

    check_div((v * U256::from_u32(q)).wrapping_shl(shift) + r, v);
}

#[test]
fn div_matches_oracle_when_the_digit_estimate_is_clamped() {
    // `r1 < v0` keeps `R` below `v * B`; `r1 + v1` is the partial remainder
    // the clamp hands to the correction loop, and the big values push it past
    // `B`, where the correction test is vacuous.
    for v1 in [0x8000_0000u32, 0x8000_0001, 0xc000_0001, u32::MAX] {
        for v0 in [1u32, 2, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
            for r1 in [0, 1, v0 / 2, v0 - 1] {
                for r0 in [0u32, 1, 0x8000_0000, u32::MAX] {
                    for q in [1u32, 2, 0x8000_0000, 0xffff_fffe, u32::MAX] {
                        // Two-limb divisor: R = [r0, r1, v1].
                        check_div_from_limbs(&[v0, v1], &[r0, r1, v1], q);

                        // Three-limb divisor: R = [r0, r1, r2, v1], where
                        // `r2 < vm` is what keeps `R < v * B`.
                        check_div_from_limbs(
                            &[r0, v0, v1],
                            &[r0, r1, v0 - 1, v1],
                            q,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn div_rem_small_matches_general_div() {
    let mut rng = Rng(0xf00d_f00d_f00d_f00d);
    for limbs in 1..=4 {
        for _ in 0..2000 {
            let n = rng.next_u256(limbs);
            let d = (rng.next_u64() as u32) | 1;
            let (q, r) = n.div_rem_small(d);
            let (eq, er) = n.div_rem(U256::from_u32(d));
            assert_eq!((q, U256::from_u32(r)), (eq, er), "{n:?} / {d}");
        }
    }
    // Powers of ten, which is what Display drives through this path.
    for d in [10u32, 1_000, 1_000_000_000] {
        let n = U256::from(u128::MAX);
        let (q, r) = n.div_rem_small(d);
        assert_eq!((q, U256::from_u32(r)), n.div_rem(U256::from_u32(d)));
    }
}

#[test]
fn div_matches_oracle_on_the_shapes_the_swap_suite_uses() {
    // mul-then-divide, as in mint_lp / mint_fee / the price accumulator.
    let cases: [(u128, u128, u128); 5] = [
        (1, 1, 1),
        (u128::MAX, u128::MAX, u128::MAX),
        (i128::MAX as u128, 3, 7),
        (1_000_000_000, 1_000_000_000, 999_999_937),
        (u128::MAX, 1, 2),
    ];
    for (a, b, c) in cases {
        check_div(U256::widening_mul(a, b), U256::from(c));
    }
    // (a << 112) / b, the UQ112x112 accumulator shape.
    for (a, b) in [
        (1u128, 1u128),
        (i128::MAX as u128, 3),
        (7, i128::MAX as u128),
    ] {
        check_div(U256::from(a).wrapping_shl(112), U256::from(b));
    }
}

#[test]
fn checked_div_reports_zero_divisor() {
    assert_eq!(U256::ONE.checked_div(U256::ZERO), None);
    assert_eq!(U256::ONE.checked_rem(U256::ZERO), None);
    assert_eq!(U256::ONE.checked_div_rem(U256::ZERO), None);
    assert_eq!(U256::ZERO.checked_div(U256::ZERO), None);
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn div_by_zero_panics() {
    let _ = U256::ONE / U256::ZERO;
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn div_rem_small_by_zero_panics() {
    let _ = U256::ONE.div_rem_small(0);
}

// ---------------------------------------------------------------------------
// Square root
// ---------------------------------------------------------------------------

fn check_sqrt(n: U256) {
    assert_eq!(
        to_oracle(n.integer_sqrt()),
        to_oracle(n).integer_sqrt(),
        "sqrt({n:?})"
    );
}

#[test]
fn sqrt_matches_oracle_on_small_values() {
    for i in 0u64..1024 {
        check_sqrt(U256::from_u64(i));
    }
}

#[test]
fn sqrt_matches_oracle_on_perfect_squares_and_neighbours() {
    for i in 0u64..512 {
        let sq = U256::from_u64(i) * U256::from_u64(i);
        check_sqrt(sq);
        if !sq.is_zero() {
            check_sqrt(sq - U256::ONE);
        }
        check_sqrt(sq + U256::ONE);
    }
}

#[test]
fn sqrt_matches_oracle_at_boundaries() {
    for n in boundary_values() {
        check_sqrt(n);
    }
}

#[test]
fn sqrt_matches_oracle_on_products_of_u128s() {
    let cases: [(u128, u128); 6] = [
        (1, 1),
        (3, 7),
        (1_000_000, 1_000_000),
        (u128::MAX, 1),
        (u128::MAX, u128::MAX),
        (i128::MAX as u128, i128::MAX as u128),
    ];
    for (a, b) in cases {
        check_sqrt(U256::widening_mul(a, b));
    }
}

#[test]
fn sqrt_result_is_the_true_floor() {
    let mut rng = Rng(0xc0ff_eec0_ffee_c0ff);
    let mut vals: Vec<U256> = (0u64..2048)
        .map(|i| U256::from_u64(i) * U256::from(7) + U256::from(3))
        .collect();
    for _ in 0..2000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        let r = n.integer_sqrt();
        let (sq, overflow) = r.overflowing_mul(r);
        assert!(!overflow && sq <= n, "{r:?}^2 > {n:?}");
        let next = r + U256::ONE;
        let (next_sq, overflow) = next.overflowing_mul(next);
        assert!(overflow || next_sq > n, "({r:?}+1)^2 <= {n:?}");
    }
}

// ---------------------------------------------------------------------------
// mul_div_u128
// ---------------------------------------------------------------------------

#[test]
fn mul_div_u128_is_exact() {
    let mut rng = Rng(0x3141_5926_5358_9793);
    let mut cases: Vec<(u128, u128, u128)> = vec![
        (1, 1, 1),
        (u128::MAX, u128::MAX, u128::MAX),
        (i128::MAX as u128, 3, 7),
        (1_000_000_000, 1_000_000_000, 999_999_937),
        (0, u128::MAX, 7),
    ];
    for _ in 0..5000 {
        cases.push((rng.next_u128(), rng.next_u128(), rng.next_u128() | 1));
    }

    for (a, b, d) in cases {
        // The oracle's 256 bits hold `a * b` exactly, so this is the true
        // quotient -- which may or may not fit back into a u128.
        let expected = (O::from(a) * O::from(b)) / O::from(d);
        let fits = expected <= O::from(u128::MAX);

        assert_eq!(
            checked_mul_div_u128(a, b, d),
            fits.then(|| expected.as_u128()),
            "{a} * {b} / {d}"
        );
        if fits {
            assert_eq!(
                O::from(mul_div_u128(a, b, d)),
                expected,
                "{a} * {b} / {d}"
            );
        }
    }
}

#[test]
fn checked_mul_div_u128_reports_failures() {
    assert_eq!(checked_mul_div_u128(1, 1, 0), None);
    // Quotient exceeds u128.
    assert_eq!(checked_mul_div_u128(u128::MAX, u128::MAX, 1), None);
    assert_eq!(checked_mul_div_u128(u128::MAX, 2, 1), None);
    // Exactly representable.
    assert_eq!(checked_mul_div_u128(u128::MAX, 1, 1), Some(u128::MAX));
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn mul_div_u128_by_zero_panics() {
    let _ = mul_div_u128(1, 1, 0);
}

#[test]
#[should_panic(expected = "quotient exceeds u128")]
fn mul_div_u128_overflow_panics() {
    let _ = mul_div_u128(u128::MAX, u128::MAX, 1);
}

// ---------------------------------------------------------------------------
// Operators track `overflow-checks` exactly as u128 does
// ---------------------------------------------------------------------------

/// The `should_panic` tests below are vacuous without overflow checks on.
#[test]
fn tests_run_with_overflow_checks_on() {
    const { assert!(cfg!(overflow_checks)) };
}

/// The other half: with checks off, `U256` wraps like `u128`.
#[test]
fn wrapping_matches_u128_semantics() {
    // `u128` wraps identically under the same flag, which is the property the
    // operators exist to mirror.
    assert_eq!(U256::MAX.wrapping_add(U256::ONE), U256::ZERO);
    assert_eq!(u128::MAX.wrapping_add(1), 0);
    assert_eq!(U256::ZERO.wrapping_sub(U256::ONE), U256::MAX);
    assert_eq!(0u128.wrapping_sub(1), u128::MAX);
}

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn add_overflow_panics_under_overflow_checks() {
    let _ = U256::MAX + U256::ONE;
}

#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
fn sub_overflow_panics_under_overflow_checks() {
    let _ = U256::ZERO - U256::ONE;
}

#[test]
#[should_panic(expected = "attempt to multiply with overflow")]
fn mul_overflow_panics_under_overflow_checks() {
    let _ = U256::MAX * U256::from(2);
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

#[test]
fn display_matches_oracle() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x2718_2818_2845_9045);
    for _ in 0..2000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        assert_eq!(format!("{n}"), format!("{}", to_oracle(n)), "{n:?}");
    }
}

#[test]
fn from_str_round_trips_display() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x1618_0339_8874_9895);
    for _ in 0..2000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        let s = format!("{n}");
        assert_eq!(s.parse::<U256>().unwrap(), n, "{s}");
    }
}

#[test]
fn from_str_rejects_bad_input() {
    use thruput_u256::ParseU256Error;
    assert_eq!("".parse::<U256>(), Err(ParseU256Error::Empty));
    assert_eq!("12a".parse::<U256>(), Err(ParseU256Error::InvalidDigit));
    assert_eq!("-1".parse::<U256>(), Err(ParseU256Error::InvalidDigit));
    // 10^78, comfortably past the 256-bit maximum of ~1.16 * 10^77. (The
    // oracle cannot represent 2^256 to build the exact boundary string.)
    let too_big = format!("1{}", "0".repeat(78));
    assert_eq!(too_big.parse::<U256>(), Err(ParseU256Error::Overflow));
    // The maximum itself must still parse.
    let max = format!("{}", U256::MAX);
    assert_eq!(max.parse::<U256>(), Ok(U256::MAX));
}

#[test]
fn hex_matches_oracle() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x4142_1356_2373_0951);
    for _ in 0..1000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        assert_eq!(format!("{n:x}"), format!("{:x}", to_oracle(n)), "{n:?}");
        assert_eq!(format!("{n:X}"), format!("{n:x}").to_uppercase());
        // The alternate flag adds the prefix, as for the primitives.
        assert_eq!(format!("{n:#x}"), format!("0x{n:x}"));
        // Debug always prefixes, and is available without the `fmt` feature.
        assert_eq!(format!("{n:?}"), format!("0x{n:x}"));
    }
}

// ---------------------------------------------------------------------------
// Byte and XDR representation
// ---------------------------------------------------------------------------

#[test]
fn be_bytes_round_trip_and_match_oracle() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for _ in 0..2000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        let bytes = n.to_be_bytes();
        assert_eq!(U256::from_be_bytes(bytes), n);

        assert_eq!(bytes, to_oracle(n).to_big_endian(), "{n:?}");
    }
}

#[test]
fn xdr_round_trips() {
    let mut vals = boundary_values();
    let mut rng = Rng(0xfedc_ba98_7654_3210);
    for _ in 0..2000 {
        vals.push(rng.next_u256(4));
    }
    for n in vals {
        let bytes = xdrs::txs::encode(&n).expect("encode");
        assert_eq!(bytes.len(), 32, "{n:?}");
        let decoded: U256 = xdrs::txs::decode(&bytes).expect("decode");
        assert_eq!(decoded, n, "{n:?}");
    }
}

/// Pins the limb order on the wire. Encoding is most-significant-limb-first,
/// which makes the 32 bytes the value in big-endian order -- the same
/// convention xdrs uses for `u128`'s `(high, low)` halves. Reversing the limb
/// order would still round-trip, so only a fixed vector catches it.
#[test]
fn xdr_encoding_is_big_endian() {
    let n = U256::from(0x0102_0304_0506_0708);
    let bytes = xdrs::txs::encode(&n).expect("encode");

    let mut expected = [0u8; 32];
    expected[24..].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(bytes, expected);
    assert_eq!(bytes, n.to_be_bytes());

    // And the maximum is all-ones.
    let max = xdrs::txs::encode(&U256::MAX).expect("encode");
    assert_eq!(max, [0xffu8; 32]);
}

// ---------------------------------------------------------------------------
// Ergonomics
// ---------------------------------------------------------------------------

/// An unsuffixed literal has to infer against `Mul<u32>` without an
/// annotation, which stops being true if a second integer `Mul` impl is added.
#[test]
fn integer_literals_and_operators_need_no_annotations() {
    let a = U256::from(5);

    assert_eq!(a * 997, U256::from(4985));
    assert_eq!(a + U256::ONE, U256::from(6));
    assert_eq!(a - U256::ONE, U256::from(4));
    assert_eq!(a << 112, U256::ONE.wrapping_shl(112) * 5);
    assert_eq!(a >> 1, U256::from(2));
    assert_eq!(a / U256::from(2), U256::from(2));
    assert_eq!(a % U256::from(2), U256::ONE);

    // Assign forms, including the u32 rhs.
    let mut b = a;
    b += U256::ONE;
    b *= 2;
    b -= U256::ONE;
    assert_eq!(b, U256::from(11));

    // The shapes the swap suite writes today.
    let amount_in = 1_000u128;
    let reserve_out = 5_000u128;
    let fee = U256::from(amount_in) * 997;
    let numerator = fee * U256::from(reserve_out);
    let denominator = U256::from(amount_in) * 1000 + fee;
    assert!(numerator / denominator > U256::ZERO);
}

/// Fixed-width hex is the ordinary spelling for a 256-bit word, so ignoring
/// the width flag would produce malformed output where it matters most.
#[test]
fn formatting_honours_width_and_padding_flags() {
    let n = U256::from(0x1f);

    // Zero-padded fixed width -- the Ethereum-style 32-byte word.
    assert_eq!(format!("{n:064x}"), format!("{:062}1f", 0));
    assert_eq!(format!("{n:064x}").len(), 64);
    assert_eq!(format!("{n:#066x}").len(), 66);
    assert!(format!("{n:#066x}").starts_with("0x0"));

    // Alignment and fill.
    assert_eq!(format!("{n:>6x}"), "    1f");
    assert_eq!(format!("{n:<6x}"), "1f    ");
    assert_eq!(format!("{n:*^6x}"), "**1f**");
    assert_eq!(format!("{n:06X}"), "00001F");

    // Decimal takes the same treatment.
    assert_eq!(format!("{n:>6}"), "    31");
    assert_eq!(format!("{n:06}"), "000031");
    assert_eq!(format!("{n:*^6}"), "**31**");

    // Zero is not a special case.
    assert_eq!(format!("{:04x}", U256::ZERO), "0000");
    assert_eq!(format!("{:04}", U256::ZERO), "0000");

    // And the flagless spellings are unchanged.
    assert_eq!(format!("{n:x}"), "1f");
    assert_eq!(format!("{n:#x}"), "0x1f");
    assert_eq!(format!("{n}"), "31");

    // U256::MAX is exactly 64 hex digits, so width 64 must not pad it.
    assert_eq!(format!("{:064x}", U256::MAX), "f".repeat(64));
}
