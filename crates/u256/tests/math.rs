// `uint::construct_uint!` trips this in its own expansion; the other suites
// carry the same allow where they invoke the macro.
#![allow(clippy::manual_div_ceil)]

//! The `U256` half of the OpenZeppelin math surface, against oracles.
//!
//! Two oracles, chosen per operation:
//!
//! * `u128`, for everything whose operands and result fit in 128 bits. It is
//!   the specification -- these functions exist to behave like the primitives
//!   at a wider width.
//! * A 512-bit `uint::construct_uint!`, for the full-precision family
//!   (`mul_div`, `mul_shr`, `mul_mod`), whose whole point is an intermediate
//!   that no 256-bit oracle can hold.

use thruput_u256::{Rounding, U256};

uint::construct_uint! {
    /// The wide oracle: eight 64-bit limbs, enough to hold any product of two
    /// `U256`s exactly.
    pub struct O512(8);
}

fn wide(n: U256) -> O512 {
    O512::from_big_endian(&{
        let mut b = [0u8; 64];
        b[32..].copy_from_slice(&n.to_be_bytes());
        b
    })
}

/// Narrows an oracle value back, or `None` if it needs more than 256 bits.
fn narrow(o: O512) -> Option<U256> {
    let bytes = o.to_big_endian();
    if bytes[..32].iter().any(|b| *b != 0) {
        return None;
    }
    Some(U256::from_be_bytes(bytes[32..].try_into().unwrap()))
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

fn boundary_values() -> Vec<U256> {
    let mut v = vec![
        U256::ZERO,
        U256::ONE,
        U256::from(2),
        U256::from(10),
        U256::MAX,
        U256::MAX - U256::ONE,
        U256::from(u64::MAX as u128),
        U256::from(u64::MAX as u128 + 1),
        U256::from(u128::MAX),
    ];
    for shift in [1u32, 31, 32, 63, 64, 95, 127, 128, 191, 192, 254, 255] {
        let x = U256::ONE.wrapping_shl(shift);
        v.push(x);
        v.push(x - U256::ONE);
        v.push(x + U256::ONE);
    }
    v
}

const ROUNDINGS: [Rounding; 4] = [
    Rounding::Floor,
    Rounding::Ceil,
    Rounding::Trunc,
    Rounding::Expand,
];

// ---------------------------------------------------------------------------
// Comparison, averaging, ceiling division
// ---------------------------------------------------------------------------

#[test]
fn min_max_and_clamp_match_ord() {
    let vals = boundary_values();
    for &a in &vals {
        for &b in &vals {
            // Inherent and trait spellings agree, which is the point of
            // shadowing them at all.
            assert_eq!(a.min(b), Ord::min(a, b), "min({a:?}, {b:?})");
            assert_eq!(a.max(b), Ord::max(a, b), "max({a:?}, {b:?})");
            assert_eq!(a.min(b), if a < b { a } else { b });
            assert_eq!(a.max(b), if a > b { a } else { b });

            let (lo, hi) = (Ord::min(a, b), Ord::max(a, b));
            for &x in vals.iter().take(12) {
                assert_eq!(x.clamp(lo, hi), Ord::clamp(x, lo, hi));
            }
        }
    }
}

#[test]
#[should_panic(expected = "min must be less than or equal to max")]
fn clamp_rejects_an_inverted_range() {
    let _ = U256::ONE.clamp(U256::from(3), U256::from(2));
}

#[test]
fn average_matches_u128_and_never_overflows() {
    let mut rng = Rng(0x1111_2222_3333_4444);
    for _ in 0..5000 {
        let a = rng.next_u128() >> 1;
        let b = rng.next_u128() >> 1;
        // Both halved, so the u128 sum is exact and is the specification.
        assert_eq!(
            U256::from(a).average(U256::from(b)),
            U256::from((a + b) / 2),
            "average({a}, {b})"
        );
    }

    // The case the bit trick exists for: a 256-bit sum that does not fit.
    assert_eq!(U256::MAX.average(U256::MAX), U256::MAX);
    assert_eq!(
        U256::MAX.average(U256::MAX - U256::ONE),
        U256::MAX - U256::ONE
    );
    assert_eq!(U256::MAX.average(U256::ZERO), U256::MAX >> 1u32);
    // Rounds toward zero, so an odd sum loses the half.
    assert_eq!(U256::ONE.average(U256::from(2)), U256::ONE);
}

#[test]
fn ceil_div_matches_u128() {
    let mut rng = Rng(0x5555_6666_7777_8888);
    let mut cases: Vec<(u128, u128)> = vec![
        (0, 1),
        (1, 1),
        (1, 2),
        (2, 1),
        (u128::MAX, 1),
        (u128::MAX, 2),
    ];
    for _ in 0..5000 {
        cases.push((rng.next_u128(), rng.next_u128().max(1)));
    }
    for (a, b) in cases {
        let expect = a.div_ceil(b);
        assert_eq!(
            U256::from(a).ceil_div(U256::from(b)),
            U256::from(expect),
            "ceil_div({a}, {b})"
        );
        assert_eq!(
            U256::from(a).checked_ceil_div(U256::from(b)),
            Some(U256::from(expect))
        );
    }

    // The largest result there is, which the `(a - 1) / b + 1` form must not
    // overflow past.
    assert_eq!(U256::MAX.ceil_div(U256::ONE), U256::MAX);
    assert_eq!(
        U256::MAX.ceil_div(U256::from(2)),
        (U256::MAX >> 1u32) + U256::ONE
    );
}

#[test]
fn ceil_div_reports_a_zero_divisor() {
    assert_eq!(U256::ONE.checked_ceil_div(U256::ZERO), None);
    // Zero over zero is still a zero divisor, not a zero answer.
    assert_eq!(U256::ZERO.checked_ceil_div(U256::ZERO), None);
    assert_eq!(U256::ZERO.checked_ceil_div(U256::ONE), Some(U256::ZERO));
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn ceil_div_by_zero_panics() {
    let _ = U256::ONE.ceil_div(U256::ZERO);
}

#[test]
fn div_rounding_matches_the_remainder() {
    let mut rng = Rng(0x9999_aaaa_bbbb_cccc);
    for _ in 0..2000 {
        let a = rng.next_u256(4);
        let b = rng.next_u256(2);
        if b.is_zero() {
            continue;
        }
        let (q, r) = a.div_rem(b);
        for rounding in ROUNDINGS {
            let up = rounding.unsigned_rounds_up() && !r.is_zero();
            assert_eq!(
                a.checked_div_rounding(b, rounding),
                Some(if up { q + U256::ONE } else { q }),
                "{a:?} / {b:?} {rounding:?}"
            );
        }
    }
    assert_eq!(
        U256::ONE.checked_div_rounding(U256::ZERO, Rounding::Ceil),
        None
    );
}

// ---------------------------------------------------------------------------
// Full-precision multiply-divide
// ---------------------------------------------------------------------------

/// The 512-bit product itself, before any division reads it.
#[test]
fn widening_mul_u256_matches_the_wide_oracle() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x0f0f_0f0f_0f0f_0f0f);
    for limbs in 1..=4 {
        for _ in 0..500 {
            vals.push(rng.next_u256(limbs));
        }
    }
    for a in &vals {
        for b in vals.iter().take(40) {
            let (lo, hi) = a.widening_mul_u256(*b);
            let got = wide(hi) * (O512::one() << 256) + wide(lo);
            assert_eq!(got, wide(*a) * wide(*b), "{a:?} * {b:?}");
        }
    }
}

#[test]
fn mul_div_matches_the_wide_oracle() {
    let mut rng = Rng(0x2222_3333_4444_5555);
    let mut checked = 0usize;
    for x_limbs in 1..=4 {
        for y_limbs in 1..=4 {
            for d_limbs in 1..=4 {
                for _ in 0..300 {
                    let x = rng.next_u256(x_limbs);
                    let y = rng.next_u256(y_limbs);
                    let d = rng.next_u256(d_limbs);
                    if d.is_zero() {
                        continue;
                    }
                    let expect = narrow((wide(x) * wide(y)) / wide(d));
                    assert_eq!(
                        U256::checked_mul_div(x, y, d),
                        expect,
                        "mul_div({x:?}, {y:?}, {d:?})"
                    );
                    checked += 1;
                }
            }
        }
    }
    // The sweep must actually reach both answers, or it proves nothing.
    assert!(checked > 10_000);
}

#[test]
fn mul_div_matches_the_wide_oracle_on_boundaries() {
    let vals = boundary_values();
    let mut overflows = 0usize;
    let mut fits = 0usize;
    for x in &vals {
        for y in &vals {
            for d in vals.iter().take(24) {
                if d.is_zero() {
                    assert_eq!(U256::checked_mul_div(*x, *y, *d), None);
                    continue;
                }
                let expect = narrow((wide(*x) * wide(*y)) / wide(*d));
                assert_eq!(
                    U256::checked_mul_div(*x, *y, *d),
                    expect,
                    "mul_div({x:?}, {y:?}, {d:?})"
                );
                if expect.is_some() {
                    fits += 1;
                } else {
                    overflows += 1;
                }
            }
        }
    }
    assert!(
        fits > 1000 && overflows > 100,
        "{fits} fit, {overflows} did not"
    );
}

/// The single-limb divisor takes a different path (chained `div_rem_small`
/// rather than Knuth D), so it needs its own sweep.
#[test]
fn mul_div_by_a_single_limb_divisor_matches_the_wide_oracle() {
    let mut rng = Rng(0x6666_7777_8888_9999);
    for _ in 0..5000 {
        let x = rng.next_u256(4);
        let y = rng.next_u256(4);
        let d = U256::from_u32((rng.next_u64() as u32).max(1));
        assert_eq!(
            U256::checked_mul_div(x, y, d),
            narrow((wide(x) * wide(y)) / wide(d)),
            "mul_div({x:?}, {y:?}, {d:?})"
        );
    }
}

#[test]
fn mul_div_agrees_with_the_u128_entry_point() {
    let mut rng = Rng(0xaaaa_bbbb_cccc_dddd);
    for _ in 0..5000 {
        let a = rng.next_u128();
        let b = rng.next_u128();
        let d = rng.next_u128().max(1);
        let via_u128 = thruput_u256::checked_mul_div_u128(a, b, d);
        let via_u256 =
            U256::checked_mul_div(U256::from(a), U256::from(b), U256::from(d));
        // The u128 entry point additionally requires the quotient to fit a
        // u128; where it does, the two must agree exactly.
        match via_u128 {
            Some(v) => assert_eq!(via_u256, Some(U256::from(v))),
            None => assert!(
                via_u256.is_none_or(|q| u128::try_from(q).is_err()),
                "{a} * {b} / {d}"
            ),
        }
    }
}

#[test]
fn mul_div_rounding_matches_the_wide_oracle() {
    let mut rng = Rng(0xdddd_eeee_ffff_0000);
    for _ in 0..3000 {
        let x = rng.next_u256(3);
        let y = rng.next_u256(3);
        let d = rng.next_u256(3);
        if d.is_zero() {
            continue;
        }
        let product = wide(x) * wide(y);
        let exact = (product % wide(d)).is_zero();
        let floor = product / wide(d);
        for rounding in ROUNDINGS {
            let up = rounding.unsigned_rounds_up() && !exact;
            let expect = narrow(if up { floor + O512::one() } else { floor });
            assert_eq!(
                U256::checked_mul_div_rounding(x, y, d, rounding),
                expect,
                "mul_div({x:?}, {y:?}, {d:?}) {rounding:?}"
            );
        }
    }
}

/// Rounding up can push a quotient that just fit out of range again.
#[test]
fn mul_div_rounding_up_reports_its_own_overflow() {
    // MAX * MAX / MAX is exactly MAX -- no rounding, no overflow.
    assert_eq!(
        U256::checked_mul_div_rounding(
            U256::MAX,
            U256::MAX,
            U256::MAX,
            Rounding::Ceil
        ),
        Some(U256::MAX)
    );
    // One less in the divisor leaves MAX with a remainder, and MAX + 1 is not
    // representable.
    let d = U256::MAX - U256::ONE;
    let (q, r) = {
        let p = wide(U256::MAX) * wide(U256::MAX);
        (narrow(p / wide(d)), p % wide(d))
    };
    assert_eq!(q, None, "the floor quotient already overflows");
    assert!(!r.is_zero());
    assert_eq!(
        U256::checked_mul_div_rounding(
            U256::MAX,
            U256::MAX,
            d,
            Rounding::Ceil
        ),
        None
    );

    // A quotient of exactly MAX with a nonzero remainder: floor fits, ceil
    // does not.
    let x = U256::MAX;
    let y = U256::from(2);
    let d = U256::from(2);
    assert_eq!(U256::checked_mul_div(x, y, d), Some(U256::MAX));
    let d = U256::from(2) + U256::ONE;
    let floor = narrow((wide(x) * wide(y)) / wide(d)).unwrap();
    assert_eq!(U256::checked_mul_div(x, y, d), Some(floor));
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn mul_div_by_zero_panics() {
    let _ = U256::mul_div(U256::ONE, U256::ONE, U256::ZERO);
}

#[test]
#[should_panic(expected = "mul_div quotient exceeds U256")]
fn mul_div_overflow_panics() {
    let _ = U256::mul_div(U256::MAX, U256::MAX, U256::ONE);
}

/// Every fallible operation ships as a `checked_` form and a panicking one.
/// `add_mod` and `div_rounding` were the two that shipped without a partner;
/// this pins the pairing so the next addition does not repeat it.
#[test]
fn panicking_and_checked_forms_agree() {
    let (a, b, m) = (U256::from(7), U256::from(9), U256::from(5));
    assert_eq!(
        U256::add_mod(a, b, m),
        U256::checked_add_mod(a, b, m).unwrap()
    );
    assert_eq!(
        U256::mul_mod(a, b, m),
        U256::checked_mul_mod(a, b, m).unwrap()
    );
    assert_eq!(a.ceil_div(b), a.checked_ceil_div(b).unwrap());
    assert_eq!(
        U256::mul_div(a, b, m),
        U256::checked_mul_div(a, b, m).unwrap()
    );
    for rounding in ROUNDINGS {
        assert_eq!(
            a.div_rounding(b, rounding),
            a.checked_div_rounding(b, rounding).unwrap(),
            "{rounding:?}"
        );
        assert_eq!(
            U256::mul_div_rounding(a, b, m, rounding),
            U256::checked_mul_div_rounding(a, b, m, rounding).unwrap()
        );
        assert_eq!(
            U256::mul_shr_rounding(a, b, 3, rounding),
            U256::checked_mul_shr_rounding(a, b, 3, rounding).unwrap()
        );
    }
    assert_eq!(
        U256::mul_shr(a, b, 3),
        U256::checked_mul_shr(a, b, 3).unwrap()
    );
    assert_eq!(
        U256::mod_exp(a, b, m),
        U256::checked_mod_exp(a, b, m).unwrap()
    );
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn add_mod_by_zero_panics() {
    let _ = U256::add_mod(U256::ONE, U256::ONE, U256::ZERO);
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn div_rounding_by_zero_panics() {
    let _ = U256::ONE.div_rounding(U256::ZERO, Rounding::Ceil);
}

#[test]
#[should_panic(expected = "mul_shr result exceeds U256")]
fn mul_shr_overflow_panics() {
    let _ = U256::mul_shr(U256::MAX, U256::MAX, 8);
}

#[test]
#[should_panic(expected = "attempt to shift right with overflow")]
fn mul_shr_past_the_width_panics() {
    let _ = U256::mul_shr(U256::ONE, U256::ONE, 256);
}

#[test]
fn mul_shr_matches_the_wide_oracle() {
    let mut rng = Rng(0x1357_9bdf_0246_8ace);
    for _ in 0..3000 {
        let x = rng.next_u256(4);
        let y = rng.next_u256(4);
        let n = (rng.next_u64() % 256) as u32;
        let product = wide(x) * wide(y);
        let expect = narrow(product >> n);
        assert_eq!(
            U256::checked_mul_shr(x, y, n),
            expect,
            "mul_shr({x:?}, {y:?}, {n})"
        );

        let exact = (product & ((O512::one() << n) - O512::one())).is_zero();
        for rounding in ROUNDINGS {
            let up = rounding.unsigned_rounds_up() && !exact;
            let want = narrow(if up {
                (product >> n) + O512::one()
            } else {
                product >> n
            });
            assert_eq!(
                U256::checked_mul_shr_rounding(x, y, n, rounding),
                want,
                "mul_shr({x:?}, {y:?}, {n}) {rounding:?}"
            );
            // The panicking form agrees wherever the checked one answers.
            if let Some(want) = want {
                assert_eq!(
                    U256::mul_shr_rounding(x, y, n, rounding),
                    want,
                    "mul_shr({x:?}, {y:?}, {n}) {rounding:?}"
                );
            }
        }
    }
}

#[test]
fn mul_shr_edges() {
    // A zero-bit shift is the plain product, and overflows like one.
    assert_eq!(
        U256::checked_mul_shr(U256::from(6), U256::from(7), 0),
        Some(U256::from(42))
    );
    assert_eq!(U256::checked_mul_shr(U256::MAX, U256::from(2), 0), None);
    assert_eq!(
        U256::checked_mul_shr(U256::MAX, U256::from(2), 1),
        Some(U256::MAX)
    );
    // 256 and beyond has no `uint8` spelling in Solidity; here it is rejected
    // rather than masked, since the operators' masking would be a wrong
    // answer rather than a wrapped one.
    assert_eq!(U256::checked_mul_shr(U256::MAX, U256::MAX, 256), None);
    // `MAX * MAX >> 255` is about `2^257`, so even the largest in-range shift
    // does not rescue it.
    assert_eq!(U256::checked_mul_shr(U256::MAX, U256::MAX, 255), None);
    // A product that does fit after the same shift.
    assert_eq!(
        U256::checked_mul_shr(U256::MAX, U256::ONE << 255u32, 255),
        Some(U256::MAX)
    );
}

// ---------------------------------------------------------------------------
// Modular arithmetic
// ---------------------------------------------------------------------------

#[test]
fn add_mod_and_mul_mod_match_the_wide_oracle() {
    let mut rng = Rng(0xfeed_beef_cafe_babe);
    for _ in 0..4000 {
        let x = rng.next_u256(4);
        let y = rng.next_u256(4);
        let m_limbs = 1 + (rng.next_u64() % 4) as usize;
        let m = rng.next_u256(m_limbs);
        if m.is_zero() {
            continue;
        }
        assert_eq!(
            U256::checked_mul_mod(x, y, m),
            narrow((wide(x) * wide(y)) % wide(m)),
            "mul_mod({x:?}, {y:?}, {m:?})"
        );
        assert_eq!(
            U256::checked_add_mod(x, y, m),
            narrow((wide(x) + wide(y)) % wide(m)),
            "add_mod({x:?}, {y:?}, {m:?})"
        );
    }

    // The carry case add_mod's conditional subtraction exists for.
    assert_eq!(
        U256::checked_add_mod(U256::MAX, U256::MAX, U256::MAX),
        Some(U256::ZERO)
    );
    assert_eq!(
        U256::checked_add_mod(U256::MAX - U256::ONE, U256::MAX, U256::MAX),
        Some(U256::MAX - U256::ONE)
    );
    assert_eq!(
        U256::checked_add_mod(U256::ONE, U256::ONE, U256::ZERO),
        None
    );
    assert_eq!(
        U256::checked_mul_mod(U256::ONE, U256::ONE, U256::ZERO),
        None
    );
}

#[test]
fn pow_matches_u128() {
    let mut rng = Rng(0x0246_8ace_1357_9bdf);
    for _ in 0..5000 {
        let base = (rng.next_u64() % 1000) as u128;
        let exp = (rng.next_u64() % 40) as u32;
        // Only where the u128 oracle itself does not overflow; the 256-bit
        // range past that is covered by the named cases below.
        if let Some(v) = base.checked_pow(exp) {
            assert_eq!(
                U256::from(base).checked_pow(exp),
                Some(U256::from(v)),
                "{base}^{exp}"
            );
        }
    }
    assert_eq!(U256::ZERO.pow(0), U256::ONE);
    assert_eq!(U256::from(2).pow(255), U256::ONE << 255u32);
    assert_eq!(U256::from(2).checked_pow(256), None);
    assert_eq!(U256::from(2).saturating_pow(256), U256::MAX);
    assert_eq!(U256::from(2).wrapping_pow(256), U256::ZERO);
    assert_eq!(U256::MAX.checked_pow(1), Some(U256::MAX));
    assert_eq!(U256::ONE.pow(u32::MAX), U256::ONE);
}

#[test]
fn mod_exp_matches_a_naive_loop() {
    let mut rng = Rng(0x0bad_f00d_0bad_f00d);
    for _ in 0..300 {
        let b = U256::from_u64(rng.next_u64());
        let e = U256::from_u32((rng.next_u64() % 64) as u32);
        let m = U256::from_u64(rng.next_u64().max(2));

        // Repeated multiply-mod, which shares no code with square-and-multiply.
        let mut expect = U256::ONE % m;
        let mut i = U256::ZERO;
        while i < e {
            expect = U256::mul_mod(expect, b, m);
            i += U256::ONE;
        }
        assert_eq!(
            U256::checked_mod_exp(b, e, m),
            Some(expect),
            "{b:?}^{e:?} mod {m:?}"
        );
    }

    assert_eq!(
        U256::checked_mod_exp(U256::from(3), U256::ZERO, U256::from(7)),
        Some(U256::ONE)
    );
    assert_eq!(
        U256::checked_mod_exp(U256::ZERO, U256::ZERO, U256::ONE),
        Some(U256::ZERO)
    );
    assert_eq!(
        U256::checked_mod_exp(U256::from(2), U256::from(10), U256::from(1000)),
        Some(U256::from(24))
    );
    assert_eq!(
        U256::checked_mod_exp(U256::ONE, U256::ONE, U256::ZERO),
        None
    );
}

/// Fermat: `a^(p-1) == 1 (mod p)` for a prime `p` and `a` not a multiple of it.
#[test]
fn mod_exp_satisfies_fermats_little_theorem() {
    // 2^127 - 1, a Mersenne prime, so the exponent is a full 127 bits.
    let p = (U256::ONE << 127u32) - U256::ONE;
    for a in [2u32, 3, 5, 7, 1_000_003] {
        let a = U256::from_u32(a);
        assert_eq!(U256::mod_exp(a, p - U256::ONE, p), U256::ONE, "{a:?}");
    }
}

/// Euclid, sharing no code with `inv_mod`'s extended form.
fn gcd(a: U256, b: U256) -> U256 {
    let (mut a, mut b) = (a, b);
    while !b.is_zero() {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

#[test]
fn inv_mod_inverts() {
    let mut rng = Rng(0xc0ff_ee00_c0ff_ee00);
    let mut inverted = 0usize;
    let mut refused = 0usize;
    for _ in 0..2000 {
        let n = rng.next_u256(2);
        if n < U256::from(2) {
            continue;
        }
        let a = rng.next_u256(2);
        let inv = U256::inv_mod(a, n);
        if inv.is_zero() {
            // A refusal must be a real one: `a` and `n` share a factor. (The
            // inverse is itself zero only for `n == 1`, excluded above.)
            assert_ne!(gcd(a, n), U256::ONE, "refused an invertible {a:?}");
            refused += 1;
        } else {
            assert_eq!(
                U256::mul_mod(a, inv, n),
                U256::ONE,
                "{a:?}^-1 mod {n:?}"
            );
            assert!(inv < n);
            inverted += 1;
        }
    }
    assert!(inverted > 500, "{inverted} inversions");
    // The sweep is random, so a refusal is rare but should still show up; if
    // it never does, the refusal path above is untested.
    assert!(refused > 0, "no non-invertible pair in the sweep");

    // Small cases worked by hand.
    assert_eq!(U256::inv_mod(U256::from(3), U256::from(7)), U256::from(5));
    assert_eq!(U256::inv_mod(U256::ONE, U256::from(5)), U256::ONE);
    // No inverse: a shares a factor with n.
    assert_eq!(U256::inv_mod(U256::from(2), U256::from(4)), U256::ZERO);
    assert_eq!(U256::inv_mod(U256::ZERO, U256::from(7)), U256::ZERO);
    // Degenerate moduli, matching Solidity's answers.
    assert_eq!(U256::inv_mod(U256::from(3), U256::ZERO), U256::ZERO);
    assert_eq!(U256::inv_mod(U256::from(3), U256::ONE), U256::ZERO);
}

#[test]
fn inv_mod_prime_agrees_with_inv_mod() {
    // 2^127 - 1.
    let p = (U256::ONE << 127u32) - U256::ONE;
    let mut rng = Rng(0x1234_1234_5678_5678);
    for _ in 0..20 {
        let a = rng.next_u256(1);
        if a.is_zero() {
            continue;
        }
        let inv = U256::inv_mod_prime(a, p);
        assert_eq!(inv, U256::inv_mod(a, p), "{a:?}");
        assert_eq!(U256::mul_mod(a, inv, p), U256::ONE);
    }
}

// ---------------------------------------------------------------------------
// Roots and logarithms
// ---------------------------------------------------------------------------

#[test]
fn sqrt_rounding_brackets_the_root() {
    let mut vals = boundary_values();
    let mut rng = Rng(0x3141_5926_5358_9793);
    for limbs in 1..=4 {
        for _ in 0..500 {
            vals.push(rng.next_u256(limbs));
        }
    }
    for n in vals {
        let floor = n.integer_sqrt();
        let exact = floor * floor == n;
        assert_eq!(n.sqrt_rounding(Rounding::Floor), floor, "{n:?}");
        assert_eq!(n.sqrt_rounding(Rounding::Trunc), floor, "{n:?}");
        let ceil = if exact { floor } else { floor + U256::ONE };
        assert_eq!(n.sqrt_rounding(Rounding::Ceil), ceil, "{n:?}");
        assert_eq!(n.sqrt_rounding(Rounding::Expand), ceil, "{n:?}");
    }
}

#[test]
fn logs_match_u128() {
    let mut rng = Rng(0x2718_2818_2845_9045);
    let mut cases: Vec<u128> =
        vec![0, 1, 2, 9, 10, 11, 99, 100, 101, u128::MAX];
    for _ in 0..3000 {
        cases.push(rng.next_u128());
    }
    for shift in 0..128u32 {
        cases.push(1u128 << shift);
    }
    for n in cases {
        let v = U256::from(n);
        assert_eq!(v.log2(), n.checked_ilog2().unwrap_or(0), "log2({n})");
        assert_eq!(v.log10(), n.checked_ilog10().unwrap_or(0), "log10({n})");
        assert_eq!(
            v.log256(),
            n.checked_ilog2().map(|b| b / 8).unwrap_or(0),
            "log256({n})"
        );
    }
}

#[test]
fn logs_cover_the_full_256_bit_range() {
    for shift in 0..256u32 {
        let x = U256::ONE << shift;
        assert_eq!(x.log2(), shift, "log2(2^{shift})");
        assert_eq!(x.log256(), shift / 8, "log256(2^{shift})");
        if shift > 0 {
            assert_eq!((x - U256::ONE).log2(), shift - 1);
        }
    }
    // 10^0 through 10^77, the full decimal range of a U256.
    let ten = U256::from(10);
    let mut p = U256::ONE;
    for e in 0..=77u32 {
        assert_eq!(p.log10(), e, "log10(10^{e})");

        // `log10_rounding` derives exactness from the peeling loop rather
        // than recomputing `10^r`; `pow` here is the independent oracle for
        // that, across the whole decimal range rather than a handful of
        // cases. An exact power never rounds up; its neighbours both do,
        // in opposite directions.
        assert_eq!(p.log10_rounding(Rounding::Ceil), e, "ceil log10(10^{e})");
        assert_eq!(
            (p + U256::ONE).log10_rounding(Rounding::Ceil),
            e + 1,
            "ceil log10(10^{e} + 1)"
        );
        if e > 0 {
            assert_eq!(
                (p - U256::ONE).log10_rounding(Rounding::Ceil),
                e,
                "ceil log10(10^{e} - 1)"
            );
        }
        for rounding in ROUNDINGS {
            let want = p.log10()
                + u32::from(
                    rounding.unsigned_rounds_up() && ten.pow(p.log10()) < p,
                );
            assert_eq!(
                p.log10_rounding(rounding),
                want,
                "10^{e} {rounding:?}"
            );
        }

        if e > 0 {
            assert_eq!((p - U256::ONE).log10(), e - 1, "log10(10^{e} - 1)");
        }
        // `10^e + 1` is still below `10^(e+1)`, so the log is unchanged.
        assert_eq!((p + U256::ONE).log10(), e, "log10(10^{e} + 1)");
        if e < 77 {
            p *= ten;
        }
    }
    assert_eq!(U256::MAX.log10(), 77);
    assert_eq!(U256::MAX.log2(), 255);
    assert_eq!(U256::MAX.log256(), 31);
}

#[test]
fn log_rounding_rounds_up_only_when_inexact() {
    let ten = U256::from(10);
    let cases = [
        U256::ZERO,
        U256::ONE,
        U256::from(2),
        U256::from(10),
        U256::from(100),
        U256::from(255),
        U256::from(256),
        U256::from(257),
        U256::MAX,
    ];
    for n in cases {
        for rounding in ROUNDINGS {
            let up = rounding.unsigned_rounds_up();

            let r = n.log2();
            let inexact = U256::ONE << r < n;
            assert_eq!(
                n.log2_rounding(rounding),
                r + u32::from(up && inexact),
                "log2({n:?}) {rounding:?}"
            );

            let r = n.log10();
            let inexact = ten.pow(r) < n;
            assert_eq!(
                n.log10_rounding(rounding),
                r + u32::from(up && inexact),
                "log10({n:?}) {rounding:?}"
            );

            let r = n.log256();
            let inexact = U256::ONE << (r * 8) < n;
            assert_eq!(
                n.log256_rounding(rounding),
                r + u32::from(up && inexact),
                "log256({n:?}) {rounding:?}"
            );
        }
    }

    // The named answers, so a sign error in the ladder cannot hide.
    assert_eq!(U256::from(255).log2_rounding(Rounding::Ceil), 8);
    assert_eq!(U256::from(256).log2_rounding(Rounding::Ceil), 8);
    assert_eq!(U256::from(257).log2_rounding(Rounding::Ceil), 9);
    assert_eq!(U256::from(99).log10_rounding(Rounding::Ceil), 2);
    assert_eq!(U256::from(100).log10_rounding(Rounding::Ceil), 2);
    assert_eq!(U256::from(101).log10_rounding(Rounding::Ceil), 3);
}

#[test]
fn rounding_classifies_directions() {
    assert!(!Rounding::Floor.unsigned_rounds_up());
    assert!(Rounding::Ceil.unsigned_rounds_up());
    assert!(!Rounding::Trunc.unsigned_rounds_up());
    assert!(Rounding::Expand.unsigned_rounds_up());

    // Away from zero: the two signed directions swap roles by sign.
    for (r, neg, want) in [
        (Rounding::Floor, false, false),
        (Rounding::Floor, true, true),
        (Rounding::Ceil, false, true),
        (Rounding::Ceil, true, false),
        (Rounding::Trunc, false, false),
        (Rounding::Trunc, true, false),
        (Rounding::Expand, false, true),
        (Rounding::Expand, true, true),
    ] {
        assert_eq!(r.rounds_away_from_zero(neg), want, "{r:?} neg={neg}");
    }
    // For a value that cannot be negative the two agree, which is why
    // `unsignedRoundsUp` can be one function in Solidity.
    for r in ROUNDINGS {
        assert_eq!(r.unsigned_rounds_up(), r.rounds_away_from_zero(false));
    }
    assert_eq!(Rounding::default(), Rounding::Floor);
}

// ---------------------------------------------------------------------------
// Const usability
// ---------------------------------------------------------------------------

/// The whole surface is `const fn`, which contracts lean on for tables of
/// precomputed constants. Only a compile-time evaluation proves it.
#[test]
fn the_math_surface_is_const_evaluable() {
    const WAD: U256 = U256::from(1_000_000_000_000_000_000);
    const HALF: U256 = WAD.average(U256::ZERO);
    const CEIL: U256 = WAD.ceil_div(U256::from(3));
    const SCALED: U256 = U256::mul_div(WAD, WAD, U256::from(7));
    const ROOT: U256 = WAD.sqrt_rounding(Rounding::Ceil);
    const L10: u32 = WAD.log10();
    const POW: U256 = U256::from(10).pow(18);
    const INV: U256 = U256::inv_mod(U256::from(3), U256::from(7));
    const CMP: core::cmp::Ordering = WAD.const_cmp(HALF);
    const MASK: U256 = U256::MAX.const_bitxor(U256::MAX.const_not());

    assert_eq!(HALF, WAD / U256::from(2));
    assert_eq!(CEIL, WAD.ceil_div(U256::from(3)));
    assert_eq!(SCALED, U256::mul_div(WAD, WAD, U256::from(7)));
    assert_eq!(ROOT, U256::from(1_000_000_000));
    assert_eq!(L10, 18);
    assert_eq!(POW, WAD);
    assert_eq!(INV, U256::from(5));
    assert_eq!(CMP, core::cmp::Ordering::Greater);
    assert_eq!(MASK, U256::MAX);
}
