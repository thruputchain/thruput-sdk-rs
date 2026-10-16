#![allow(clippy::manual_div_ceil)]

//! `I256` against oracles.
//!
//! Two of them, because no single one covers the type:
//!
//! * `i128` is the specification. Every operation whose operands and result
//!   fit in 128 bits must agree with it exactly -- that is what "behaves like
//!   the primitives" means, and it is the property a port from Solidity
//!   depends on.
//! * Parity's `uint`, the crate's existing unsigned oracle, pins the two's
//!   complement claim: the bits of a signed add/sub/mul/bitop must equal the
//!   bits of the unsigned one.
//!
//! Past 128 bits the identities carry the weight: the division identity, the
//! shift-versus-Euclidean-division identity, and the named edges around `MIN`,
//! where every overflow in the type lives.

use thruput_u256::{I256, ParseI256Error, Rounding, U256};

uint::construct_uint! {
    /// The unsigned oracle: four 64-bit limbs.
    pub struct O(4);
}

uint::construct_uint! {
    /// The wide oracle, for the signed `mul_div` product.
    pub struct O512(8);
}

fn to_oracle(n: U256) -> O {
    O::from_big_endian(&n.to_be_bytes())
}

fn wide(n: U256) -> O512 {
    let mut b = [0u8; 64];
    b[32..].copy_from_slice(&n.to_be_bytes());
    O512::from_big_endian(&b)
}

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

    fn next_i128(&mut self) -> i128 {
        (((self.next_u64() as u128) << 64) | (self.next_u64() as u128)) as i128
    }

    /// A value with `limbs` significant 64-bit limbs, read as signed.
    fn next_i256(&mut self, limbs: usize) -> I256 {
        let mut v = U256::ZERO;
        for _ in 0..limbs {
            v = v.wrapping_shl(64) | U256::from_u64(self.next_u64());
        }
        v.cast_signed()
    }
}

/// Interesting values: the extremes, both signs of every limb boundary, and
/// the neighbours of zero.
fn boundary_values() -> Vec<I256> {
    let mut v = vec![
        I256::ZERO,
        I256::ONE,
        I256::MINUS_ONE,
        I256::MAX,
        I256::MIN,
        I256::MAX - I256::ONE,
        I256::MIN + I256::ONE,
        I256::from(i128::MAX),
        I256::from(i128::MIN),
        u128::MAX.into(),
    ];
    for shift in [1u32, 31, 32, 63, 64, 95, 127, 128, 191, 192, 254] {
        let x = I256::ONE.wrapping_shl(shift);
        for base in [x, x - I256::ONE, x + I256::ONE] {
            v.push(base);
            v.push(base.wrapping_neg());
        }
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
// Representation and conversions
// ---------------------------------------------------------------------------

#[test]
fn constants_are_the_two_complement_extremes() {
    assert_eq!(I256::ZERO.cast_unsigned(), U256::ZERO);
    assert_eq!(I256::ONE.cast_unsigned(), U256::ONE);
    assert_eq!(I256::MINUS_ONE.cast_unsigned(), U256::MAX);
    assert_eq!(I256::MAX.cast_unsigned(), U256::MAX >> 1u32);
    assert_eq!(I256::MIN.cast_unsigned(), U256::ONE << 255u32);
    assert_eq!(I256::BITS, 256);

    assert!(I256::MIN.is_negative());
    assert!(!I256::MIN.is_positive());
    assert!(I256::MAX.is_positive());
    assert!(!I256::ZERO.is_negative());
    assert!(!I256::ZERO.is_positive());
    assert!(I256::ZERO.is_zero());

    assert_eq!(I256::MIN.signum(), I256::MINUS_ONE);
    assert_eq!(I256::ZERO.signum(), I256::ZERO);
    assert_eq!(I256::MAX.signum(), I256::ONE);

    // The sign bit is bit 255, and `bit` does not sign-extend past the width.
    assert!(I256::MINUS_ONE.bit(255));
    assert!(!I256::MAX.bit(255));
    assert!(!I256::MINUS_ONE.bit(256));
}

#[test]
fn from_primitives_matches_i128() {
    let mut rng = Rng(0x1234_5678_9abc_def0);
    for _ in 0..5000 {
        let n = rng.next_i128();
        let v = I256::from(n);
        assert_eq!(i128::try_from(v), Ok(n), "{n}");
        // Sign extension: the top limbs are all ones exactly when negative.
        assert_eq!(v.is_negative(), n < 0);
        // Through `Into`, the only route to the narrower `From` impls now
        // that the inherent `I256::from` shadows them.
        assert_eq!(I256::from_i64(n as i64), (n as i64).into());
        assert_eq!(I256::from_i32(n as i32), (n as i32).into());
        assert_eq!(I256::from_i32(n as i8 as i32), (n as i8).into());
        // Unsigned primitives all fit, so none of them can be negative here.
        let u: I256 = (n as u128).into();
        assert!(!u.is_negative());
        assert_eq!(u128::try_from(u), Ok(n as u128));
    }
    // `bool` reaches the trait impl, since the inherent `from` takes `i128`.
    assert_eq!(<I256 as From<bool>>::from(true), I256::ONE);
    assert_eq!(<I256 as From<bool>>::from(false), I256::ZERO);
    assert_eq!(
        I256::from_i128(i128::MIN).cast_unsigned(),
        (U256::ONE << 127u32).wrapping_neg()
    );
}

#[test]
fn try_from_narrower_types_rejects_out_of_range() {
    // Exactly at the boundary in both directions, for each width.
    assert_eq!(i128::try_from(I256::from(i128::MAX)), Ok(i128::MAX));
    assert_eq!(i128::try_from(I256::from(i128::MIN)), Ok(i128::MIN));
    assert!(i128::try_from(I256::from(i128::MAX) + I256::ONE).is_err());
    assert!(i128::try_from(I256::from(i128::MIN) - I256::ONE).is_err());

    assert_eq!(i64::try_from(I256::from(i64::MIN as i128)), Ok(i64::MIN));
    assert!(i64::try_from(I256::from(i64::MIN as i128 - 1)).is_err());
    assert_eq!(i32::try_from(I256::from(i32::MIN as i128)), Ok(i32::MIN));
    assert!(i32::try_from(I256::from(i32::MIN as i128 - 1)).is_err());

    // Unsigned targets refuse anything negative, however small.
    assert!(u128::try_from(I256::MINUS_ONE).is_err());
    assert!(u64::try_from(I256::MINUS_ONE).is_err());
    assert!(u32::try_from(I256::MINUS_ONE).is_err());
    let biggest: I256 = u128::MAX.into();
    assert_eq!(u128::try_from(biggest), Ok(u128::MAX));
    assert!(u64::try_from(biggest).is_err());

    // MIN and MAX fit nothing narrower.
    assert!(i128::try_from(I256::MIN).is_err());
    assert!(i128::try_from(I256::MAX).is_err());
}

#[test]
fn try_from_narrower_types_matches_i128_on_random_inputs() {
    let mut rng = Rng(0x0bad_c0de_0bad_c0de);
    for _ in 0..5000 {
        let n = rng.next_i128();
        let v = I256::from(n);
        assert_eq!(i64::try_from(v).ok(), i64::try_from(n).ok(), "{n}");
        assert_eq!(i32::try_from(v).ok(), i32::try_from(n).ok(), "{n}");
        assert_eq!(u64::try_from(v).ok(), u64::try_from(n).ok(), "{n}");
        assert_eq!(u32::try_from(v).ok(), u32::try_from(n).ok(), "{n}");
        assert_eq!(u128::try_from(v).ok(), u128::try_from(n).ok(), "{n}");
    }
}

#[test]
fn casts_between_the_signed_and_unsigned_views_are_pure_reinterpretation() {
    let mut rng = Rng(0xfeed_face_dead_beef);
    for _ in 0..5000 {
        let v = rng.next_i256(4);
        assert_eq!(v.cast_unsigned().cast_signed(), v);
        assert_eq!(v.to_limbs(), v.cast_unsigned().to_limbs());
    }

    // The checked directions refuse what the casts reinterpret.
    assert!(I256::try_from(U256::MAX).is_err());
    assert_eq!(U256::MAX.cast_signed(), I256::MINUS_ONE);
}

#[test]
fn checked_conversions_between_the_two_widths() {
    // Everything up to MAX converts; the first value past it does not.
    assert_eq!(I256::try_from(U256::ZERO), Ok(I256::ZERO));
    assert_eq!(I256::try_from(I256::MAX.cast_unsigned()), Ok(I256::MAX));
    assert!(I256::try_from(I256::MAX.cast_unsigned() + U256::ONE).is_err());
    assert!(I256::try_from(U256::MAX).is_err());

    assert_eq!(U256::try_from(I256::ZERO), Ok(U256::ZERO));
    assert_eq!(U256::try_from(I256::MAX), Ok(I256::MAX.cast_unsigned()));
    assert!(U256::try_from(I256::MINUS_ONE).is_err());
    assert!(U256::try_from(I256::MIN).is_err());
}

// ---------------------------------------------------------------------------
// Ordering
// ---------------------------------------------------------------------------

#[test]
fn ordering_matches_i128() {
    let mut rng = Rng(0x2718_2818_2845_9045);
    let mut vals: Vec<i128> = vec![0, 1, -1, i128::MAX, i128::MIN];
    for _ in 0..200 {
        vals.push(rng.next_i128());
    }
    for a in &vals {
        for b in &vals {
            assert_eq!(
                I256::from(*a).cmp(&I256::from(*b)),
                a.cmp(b),
                "cmp({a}, {b})"
            );
        }
    }
}

#[test]
fn ordering_places_every_negative_below_every_non_negative() {
    let vals = boundary_values();
    for a in &vals {
        for b in &vals {
            if a.is_negative() && !b.is_negative() {
                assert!(a < b, "{a:?} < {b:?}");
            }
            // The const and trait spellings agree.
            assert_eq!(a.const_cmp(*b), a.cmp(b), "{a:?} vs {b:?}");
        }
    }
    assert!(I256::MIN < I256::MINUS_ONE);
    assert!(I256::MINUS_ONE < I256::ZERO);
    assert!(I256::ZERO < I256::MAX);
}

// ---------------------------------------------------------------------------
// Addition, subtraction, multiplication
// ---------------------------------------------------------------------------

#[test]
fn add_sub_mul_match_i128() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut pairs: Vec<(i128, i128)> = vec![
        (0, 0),
        (i128::MAX, 1),
        (i128::MIN, -1),
        (i128::MAX, i128::MAX),
        (i128::MIN, i128::MIN),
        (-1, -1),
    ];
    for _ in 0..5000 {
        pairs.push((rng.next_i128(), rng.next_i128()));
        // Also narrow operands, where nothing overflows and the products stay
        // exact.
        pairs.push((rng.next_i128() >> 70, rng.next_i128() >> 70));
    }

    for (a, b) in pairs {
        let (x, y) = (I256::from(a), I256::from(b));

        // The sum of two i128s always fits in 256 bits, so the I256 result is
        // the exact value -- which means it converts back to an i128 exactly
        // when the i128 addition would not have overflowed. One assertion
        // covers both directions.
        assert_eq!(i128::try_from(x + y).ok(), a.checked_add(b), "{a} + {b}");
        assert_eq!(i128::try_from(x - y).ok(), a.checked_sub(b), "{a} - {b}");
        if let Some(p) = a.checked_mul(b) {
            assert_eq!(x * y, I256::from(p), "{a} * {b}");
        }
        // The product of two i128s always fits in 256 bits, so the I256
        // multiply must never report an overflow here.
        assert!(!x.overflowing_mul(y).1, "{a} * {b} overflowed I256");
    }
}

/// The two's complement claim: the bits of a signed operation are the bits of
/// the unsigned one.
#[test]
fn wrapping_arithmetic_has_the_same_bits_as_the_unsigned_oracle() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..200 {
            vals.push(rng.next_i256(limbs));
        }
    }

    for a in vals.iter().take(60) {
        for b in vals.iter().take(60) {
            let (ua, ub) =
                (to_oracle(a.cast_unsigned()), to_oracle(b.cast_unsigned()));

            assert_eq!(
                to_oracle(a.wrapping_add(*b).cast_unsigned()),
                ua.overflowing_add(ub).0,
                "{a:?} + {b:?}"
            );
            assert_eq!(
                to_oracle(a.wrapping_sub(*b).cast_unsigned()),
                ua.overflowing_sub(ub).0,
                "{a:?} - {b:?}"
            );
            assert_eq!(
                to_oracle(a.wrapping_mul(*b).cast_unsigned()),
                ua.overflowing_mul(ub).0,
                "{a:?} * {b:?}"
            );
            assert_eq!(
                to_oracle((*a & *b).cast_unsigned()),
                ua & ub,
                "{a:?} & {b:?}"
            );
            assert_eq!(to_oracle((*a | *b).cast_unsigned()), ua | ub);
            assert_eq!(to_oracle((*a ^ *b).cast_unsigned()), ua ^ ub);
        }
        assert_eq!(
            to_oracle((!*a).cast_unsigned()),
            !to_oracle(a.cast_unsigned())
        );
    }
}

#[test]
fn overflow_flags_match_i128_at_the_128_bit_width() {
    // Scaled into I256, an i128 overflow is not an I256 overflow -- so the
    // agreement to check is at I256's own width, using a 128-bit shadow of
    // the type. Shift every operand up by 128 bits: then an i128-width
    // overflow is exactly an I256-width one.
    let mut rng = Rng(0x1618_0339_8874_9895);
    for _ in 0..20_000 {
        let a = rng.next_i128();
        let b = rng.next_i128();
        let (x, y) = (
            I256::from(a).wrapping_shl(128),
            I256::from(b).wrapping_shl(128),
        );

        assert_eq!(
            x.overflowing_add(y).1,
            a.overflowing_add(b).1,
            "{a} + {b}"
        );
        assert_eq!(
            x.overflowing_sub(y).1,
            a.overflowing_sub(b).1,
            "{a} - {b}"
        );
        assert_eq!(
            x.checked_add(y).map(|v| v.wrapping_shr(128)),
            a.checked_add(b).map(I256::from),
            "{a} + {b}"
        );
        assert_eq!(
            x.saturating_add(y),
            match a.checked_add(b) {
                Some(v) => I256::from(v).wrapping_shl(128),
                None if a < 0 => I256::MIN,
                None => I256::MAX,
            },
            "{a} + {b}"
        );
    }
}

#[test]
fn multiplication_overflow_is_a_magnitude_bound() {
    // MIN * 1 is representable; MIN * -1 is not, and neither is MAX * -1 - 1.
    assert_eq!(I256::MIN.checked_mul(I256::ONE), Some(I256::MIN));
    assert_eq!(I256::MIN.checked_mul(I256::MINUS_ONE), None);
    assert_eq!(I256::MAX.checked_mul(I256::MINUS_ONE), Some(-I256::MAX));
    assert_eq!(I256::MIN.checked_mul(I256::ZERO), Some(I256::ZERO));
    assert_eq!(I256::MIN.checked_mul(I256::MIN), None);

    // The largest exact products in each direction.
    let half = I256::ONE.wrapping_shl(127);
    assert!(half.checked_mul(half).is_some());
    assert_eq!(half.checked_mul(half), Some(I256::ONE.wrapping_shl(254)));
    let root = I256::ONE.wrapping_shl(128);
    assert_eq!(root.checked_mul(root), None);
    assert_eq!(root.checked_mul(-root), None);

    // Saturation picks the side the true product is on.
    assert_eq!(I256::MAX.saturating_mul(I256::from(2)), I256::MAX);
    assert_eq!(I256::MAX.saturating_mul(I256::from(-2)), I256::MIN);
    assert_eq!(I256::MIN.saturating_mul(I256::MINUS_ONE), I256::MAX);
    assert_eq!(I256::MIN.saturating_mul(I256::from(2)), I256::MIN);
}

#[test]
fn negation_and_absolute_value() {
    let mut rng = Rng(0x3141_5926_5358_9793);
    for _ in 0..5000 {
        let n = rng.next_i128();
        let v = I256::from(n);
        assert_eq!(-v, I256::from(-n), "-{n}");
        assert_eq!(v.checked_neg(), Some(I256::from(-n)));
        assert_eq!(v.abs(), I256::from(n.abs()), "|{n}|");
        assert_eq!(v.unsigned_abs(), U256::from(n.unsigned_abs()));
        assert_eq!(v.signum(), I256::from(n.signum()));
    }

    // MIN is the one value with no negation.
    assert_eq!(I256::MIN.checked_neg(), None);
    assert_eq!(I256::MIN.wrapping_neg(), I256::MIN);
    assert_eq!(I256::MIN.saturating_neg(), I256::MAX);
    assert_eq!(I256::MIN.checked_abs(), None);
    assert_eq!(I256::MIN.wrapping_abs(), I256::MIN);
    assert_eq!(I256::MIN.saturating_abs(), I256::MAX);
    // But its magnitude is perfectly representable unsigned.
    assert_eq!(I256::MIN.unsigned_abs(), U256::ONE << 255u32);
    assert_eq!(I256::MAX.unsigned_abs(), I256::MAX.cast_unsigned());

    // `!x == -x - 1`, for every value including MIN.
    for v in boundary_values() {
        assert_eq!(!v, v.wrapping_neg().wrapping_sub(I256::ONE), "!{v:?}");
    }
}

// ---------------------------------------------------------------------------
// Division
// ---------------------------------------------------------------------------

#[test]
fn division_matches_i128() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    let mut pairs: Vec<(i128, i128)> = vec![
        (7, 2),
        (-7, 2),
        (7, -2),
        (-7, -2),
        (i128::MIN, -1),
        (i128::MIN, 1),
        (i128::MAX, -1),
        (0, 5),
        (0, -5),
        (1, i128::MIN),
    ];
    for _ in 0..10_000 {
        pairs.push((rng.next_i128(), rng.next_i128()));
        pairs.push((rng.next_i128(), rng.next_i128() >> 100));
    }

    for (a, b) in pairs {
        if b == 0 {
            continue;
        }
        let (x, y) = (I256::from(a), I256::from(b));
        // The only i128 division that overflows is MIN / -1, which does not
        // overflow at 256 bits.
        let expect_q = a.wrapping_div(b);
        let expect_r = a.wrapping_rem(b);
        if a == i128::MIN && b == -1 {
            assert_eq!(x / y, -I256::from(i128::MIN), "{a} / {b}");
            assert_eq!(x % y, I256::ZERO);
            continue;
        }
        assert_eq!(x / y, I256::from(expect_q), "{a} / {b}");
        assert_eq!(x % y, I256::from(expect_r), "{a} % {b}");
        assert_eq!(x.div_rem(y), (I256::from(expect_q), I256::from(expect_r)));
        assert_eq!(
            x.div_euclid(y),
            I256::from(a.div_euclid(b)),
            "{a}.div_euclid({b})"
        );
        assert_eq!(
            x.rem_euclid(y),
            I256::from(a.rem_euclid(b)),
            "{a}.rem_euclid({b})"
        );
    }
}

/// Truncation toward zero, spelled out, because it is the one place a signed
/// port most often goes wrong.
#[test]
fn division_truncates_toward_zero() {
    let seven = I256::from(7);
    let two = I256::from(2);
    assert_eq!(seven / two, I256::from(3));
    assert_eq!(-seven / two, I256::from(-3));
    assert_eq!(seven / -two, I256::from(-3));
    assert_eq!(-seven / -two, I256::from(3));

    // The remainder takes the sign of the dividend.
    assert_eq!(seven % two, I256::ONE);
    assert_eq!(-seven % two, I256::MINUS_ONE);
    assert_eq!(seven % -two, I256::ONE);
    assert_eq!(-seven % -two, I256::MINUS_ONE);

    // Euclidean division never leaves a negative remainder.
    assert_eq!((-seven).div_euclid(two), I256::from(-4));
    assert_eq!((-seven).rem_euclid(two), I256::ONE);
    assert_eq!((-seven).div_euclid(-two), I256::from(4));
    assert_eq!((-seven).rem_euclid(-two), I256::ONE);
}

#[test]
fn the_division_identity_holds_at_full_width() {
    let mut rng = Rng(0x4142_1356_2373_0951);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..300 {
            vals.push(rng.next_i256(limbs));
        }
    }
    for a in &vals {
        for b in vals.iter().take(40) {
            if b.is_zero() {
                continue;
            }
            let Some((q, r)) = a.checked_div_rem(*b) else {
                // The only refusal left is MIN / -1.
                assert_eq!((*a, *b), (I256::MIN, I256::MINUS_ONE));
                continue;
            };
            // `q * b + r == a`, computed wrapping so the identity is testable
            // even where `q * b` alone would leave the range.
            assert_eq!(
                q.wrapping_mul(*b).wrapping_add(r),
                *a,
                "{a:?} / {b:?} = {q:?} rem {r:?}"
            );
            // |r| < |b|, and r takes the dividend's sign.
            assert!(r.unsigned_abs() < b.unsigned_abs(), "{a:?} / {b:?}");
            if !r.is_zero() {
                assert_eq!(r.is_negative(), a.is_negative(), "{a:?} / {b:?}");
            }
        }
    }
}

#[test]
fn min_over_minus_one_is_the_only_division_overflow() {
    assert_eq!(I256::MIN.checked_div(I256::MINUS_ONE), None);
    assert_eq!(I256::MIN.checked_rem(I256::MINUS_ONE), None);
    assert_eq!(I256::MIN.wrapping_div(I256::MINUS_ONE), I256::MIN);
    assert_eq!(I256::MIN.wrapping_rem(I256::MINUS_ONE), I256::ZERO);
    // Every other MIN division is fine.
    assert_eq!(I256::MIN.checked_div(I256::ONE), Some(I256::MIN));
    assert_eq!(I256::MIN / I256::from(2), I256::MIN.wrapping_shr(1));
    assert_eq!(I256::MAX.checked_div(I256::MINUS_ONE), Some(-I256::MAX));
    // A zero divisor is refused whatever the dividend.
    assert_eq!(I256::ZERO.checked_div(I256::ZERO), None);
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn division_by_zero_panics() {
    let _ = I256::ONE / I256::ZERO;
}

/// Division overflow is the one arithmetic overflow the primitives do *not*
/// gate on `overflow-checks`: `i128::MIN / -1` panics under
/// `-C overflow-checks=off` too, because the alternative on real hardware is a
/// trap. So this panic must be unconditional, and a contract profile must not
/// quietly produce `MIN` here.
///
/// The test suite runs with checks on, so this case alone cannot prove the
/// ungated half; `division_overflow_is_not_gated_on_overflow_checks` below
/// pins the shape that would regress if the gate came back.
#[test]
#[should_panic(expected = "attempt to divide with overflow")]
fn min_over_minus_one_panics() {
    let _ = I256::MIN / I256::MINUS_ONE;
}

#[test]
#[should_panic(expected = "attempt to divide with overflow")]
fn min_rem_minus_one_panics() {
    let _ = I256::MIN % I256::MINUS_ONE;
}

/// The ungated claim, stated where a reader will find it.
///
/// `div_rem` reaches its overflow `panic!` without consulting
/// `OVERFLOW_CHECKS` — unlike `+`, `-`, `*` and unary `-`, which all do. A
/// regression that re-gated it would leave every assertion above passing (the
/// harness builds with checks on) and silently change what ships, so the
/// escape hatches are asserted here instead: they are the only in-profile way
/// to get a value out of `MIN / -1`, and they must keep working.
#[test]
fn division_overflow_is_not_gated_on_overflow_checks() {
    assert_eq!(I256::MIN.checked_div(I256::MINUS_ONE), None);
    assert_eq!(I256::MIN.checked_rem(I256::MINUS_ONE), None);
    assert_eq!(I256::MIN.wrapping_div(I256::MINUS_ONE), I256::MIN);
    assert_eq!(I256::MIN.wrapping_rem(I256::MINUS_ONE), I256::ZERO);
    assert_eq!(
        I256::MIN.overflowing_div(I256::MINUS_ONE),
        (I256::MIN, true)
    );
    assert_eq!(
        I256::MIN.overflowing_rem(I256::MINUS_ONE),
        (I256::ZERO, true)
    );

    // And the same shapes on i128, so "like the primitives" is visible rather
    // than asserted: these are total there too.
    assert_eq!(i128::MIN.checked_div(-1), None);
    assert_eq!(i128::MIN.wrapping_div(-1), i128::MIN);
    assert_eq!(i128::MIN.overflowing_div(-1), (i128::MIN, true));
}

// ---------------------------------------------------------------------------
// Shifts and bitwise
// ---------------------------------------------------------------------------

#[test]
fn shifts_match_i128() {
    let mut rng = Rng(0x1357_9bdf_0246_8ace);
    for _ in 0..5000 {
        let n = rng.next_i128();
        let v = I256::from(n);
        for shift in [0u32, 1, 7, 31, 32, 63, 64, 100, 126, 127] {
            // A left shift can leave the i128 range while staying in the
            // I256 one, so only the right shift compares directly.
            assert_eq!(
                v.wrapping_shr(shift),
                I256::from(n >> shift),
                "{n} >> {shift}"
            );
            assert_eq!(v >> shift, I256::from(n >> shift));
            assert_eq!(
                v.wrapping_shl(shift),
                I256::from(n).wrapping_mul(I256::ONE.wrapping_shl(shift)),
                "{n} << {shift}"
            );
        }
    }
}

/// An arithmetic right shift is floor division by a power of two -- which is
/// Euclidean division, not the truncating kind. This is the identity a
/// logical-shift bug breaks immediately.
#[test]
fn arithmetic_right_shift_is_floor_division() {
    let mut rng = Rng(0x2718_2818_2845_9045);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..200 {
            vals.push(rng.next_i256(limbs));
        }
    }
    for v in vals {
        for shift in [0u32, 1, 2, 31, 32, 63, 64, 127, 128, 200, 254] {
            let divisor = I256::ONE.wrapping_shl(shift);
            assert_eq!(
                v.wrapping_shr(shift),
                v.div_euclid(divisor),
                "{v:?} >> {shift}"
            );
        }
        // Shifting a negative value all the way down saturates at -1, never
        // at 0.
        if v.is_negative() {
            assert_eq!(v.wrapping_shr(255), I256::MINUS_ONE, "{v:?} >> 255");
        } else {
            assert_eq!(v.wrapping_shr(255), I256::ZERO, "{v:?} >> 255");
        }
    }
    assert_eq!(I256::MINUS_ONE >> 1u32, I256::MINUS_ONE);
    assert_eq!(I256::MIN >> 255u32, I256::MINUS_ONE);
}

#[test]
fn out_of_range_shifts_are_checked_and_masked_like_the_unsigned_side() {
    assert_eq!(I256::ONE.checked_shl(255), Some(I256::MIN));
    assert_eq!(I256::ONE.checked_shl(256), None);
    assert_eq!(I256::ONE.checked_shr(256), None);
    assert_eq!(I256::MINUS_ONE.checked_shr(255), Some(I256::MINUS_ONE));
    assert_eq!(I256::ONE.overflowing_shl(256), (I256::ONE, true));
    assert_eq!(I256::ONE.overflowing_shl(4), (I256::from(16), false));
    // Masking, not saturating: 300 % 256 == 44.
    assert_eq!(I256::ONE.wrapping_shl(300), I256::ONE.wrapping_shl(44));
}

#[test]
fn bitwise_operations_match_i128() {
    let mut rng = Rng(0x6666_7777_8888_9999);
    for _ in 0..5000 {
        let a = rng.next_i128();
        let b = rng.next_i128();
        let (x, y) = (I256::from(a), I256::from(b));
        assert_eq!(x & y, I256::from(a & b), "{a} & {b}");
        assert_eq!(x | y, I256::from(a | b), "{a} | {b}");
        assert_eq!(x ^ y, I256::from(a ^ b), "{a} ^ {b}");
        assert_eq!(!x, I256::from(!a), "!{a}");
    }
}

#[test]
fn assign_operators_agree_with_their_binary_forms() {
    let mut v = I256::from(-100);
    v += I256::from(30);
    assert_eq!(v, I256::from(-70));
    v -= I256::from(-70);
    assert_eq!(v, I256::ZERO);
    v = I256::from(-9);
    v *= I256::from(3);
    assert_eq!(v, I256::from(-27));
    v /= I256::from(4);
    assert_eq!(v, I256::from(-6));
    v %= I256::from(4);
    assert_eq!(v, I256::from(-2));
    v <<= 3u32;
    assert_eq!(v, I256::from(-16));
    v >>= 2u32;
    assert_eq!(v, I256::from(-4));
    v &= I256::from(-3);
    assert_eq!(v, I256::from(-4) & I256::from(-3));
    v |= I256::ONE;
    v ^= I256::ONE;
    assert_eq!(v, I256::from(-4) & I256::from(-3));
}

// ---------------------------------------------------------------------------
// Overflow panics
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn add_overflow_panics_under_overflow_checks() {
    let _ = I256::MAX + I256::ONE;
}

#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
fn sub_overflow_panics_under_overflow_checks() {
    let _ = I256::MIN - I256::ONE;
}

#[test]
#[should_panic(expected = "attempt to multiply with overflow")]
fn mul_overflow_panics_under_overflow_checks() {
    let _ = I256::MIN * I256::from(2);
}

#[test]
#[should_panic(expected = "attempt to negate with overflow")]
fn neg_overflow_panics_under_overflow_checks() {
    let _ = -I256::MIN;
}

#[test]
fn wrapping_forms_wrap_where_the_operators_would_panic() {
    assert_eq!(I256::MAX.wrapping_add(I256::ONE), I256::MIN);
    assert_eq!(I256::MIN.wrapping_sub(I256::ONE), I256::MAX);
    assert_eq!(I256::MIN.wrapping_mul(I256::from(2)), I256::ZERO);
    assert_eq!(I256::MAX.saturating_add(I256::ONE), I256::MAX);
    assert_eq!(I256::MIN.saturating_sub(I256::ONE), I256::MIN);
    // The same shapes on i128, so the claim "like the primitives" is visible.
    assert_eq!(i128::MAX.wrapping_add(1), i128::MIN);
    assert_eq!(i128::MIN.wrapping_sub(1), i128::MAX);
}

/// The `overflowing_*` family is what every `checked_`/`wrapping_`/
/// `saturating_` form is built on, so its *flag* is the thing a bug would hide
/// in: the wrappers either discard it or branch on it, and a flag stuck at
/// `false` would leave them all still returning plausible values. Assert the
/// pairs directly.
#[test]
fn overflowing_forms_report_their_flags() {
    // Negation and absolute value: MIN is the only overflow.
    assert_eq!(I256::ZERO.overflowing_neg(), (I256::ZERO, false));
    assert_eq!(I256::ONE.overflowing_neg(), (I256::MINUS_ONE, false));
    assert_eq!(I256::MAX.overflowing_neg(), (-I256::MAX, false));
    assert_eq!(I256::MIN.overflowing_neg(), (I256::MIN, true));

    assert_eq!(I256::MINUS_ONE.overflowing_abs(), (I256::ONE, false));
    assert_eq!(I256::MAX.overflowing_abs(), (I256::MAX, false));
    assert_eq!(I256::MIN.overflowing_abs(), (I256::MIN, true));

    // Arithmetic shift: the flag reports the mask, not a lost value.
    assert_eq!(I256::MINUS_ONE.overflowing_shr(0), (I256::MINUS_ONE, false));
    assert_eq!(I256::MIN.overflowing_shr(255), (I256::MINUS_ONE, false));
    assert_eq!(I256::ONE.overflowing_shr(256), (I256::ONE, true));
    assert_eq!(I256::from(8).overflowing_shr(2), (I256::from(2), false));

    // Exponentiation.
    assert_eq!(I256::from(3).overflowing_pow(0), (I256::ONE, false));
    assert_eq!(I256::from(-3).overflowing_pow(3), (I256::from(-27), false));
    assert!(!I256::from(2).overflowing_pow(254).1);
    assert!(I256::from(2).overflowing_pow(255).1);
    assert_eq!(I256::from(-2).overflowing_pow(255), (I256::MIN, false));
    assert!(I256::from(-2).overflowing_pow(256).1);

    // Every flag must agree with its `checked_` sibling, across the boundary
    // values -- that is the invariant the wrappers rely on.
    for v in boundary_values() {
        assert_eq!(v.overflowing_neg().1, v.checked_neg().is_none(), "{v:?}");
        assert_eq!(v.overflowing_abs().1, v.checked_abs().is_none(), "{v:?}");
        for exp in [0u32, 1, 2, 3, 8] {
            assert_eq!(
                v.overflowing_pow(exp).1,
                v.checked_pow(exp).is_none(),
                "{v:?}^{exp}"
            );
        }
        for n in [0u32, 1, 255, 256, 300] {
            assert_eq!(v.overflowing_shr(n).1, v.checked_shr(n).is_none());
            assert_eq!(v.overflowing_shl(n).1, v.checked_shl(n).is_none());
        }
        for d in [I256::ONE, I256::MINUS_ONE, I256::from(3), I256::MIN] {
            assert_eq!(
                v.overflowing_div(d).1,
                v.checked_div(d).is_none(),
                "{v:?} / {d:?}"
            );
            assert_eq!(v.overflowing_rem(d).1, v.checked_rem(d).is_none());
        }
    }
}

/// `U256`'s own negation family, added alongside `I256` and equally unasserted
/// until now.
#[test]
fn unsigned_negation_reports_its_flag() {
    assert_eq!(U256::ZERO.overflowing_neg(), (U256::ZERO, false));
    assert_eq!(U256::ONE.overflowing_neg(), (U256::MAX, true));
    assert_eq!(U256::MAX.overflowing_neg(), (U256::ONE, true));
    assert_eq!(U256::ZERO.checked_neg(), Some(U256::ZERO));
    assert_eq!(U256::ONE.checked_neg(), None);
    // The two's complement identity `-x == !x + 1`, which is what I256 leans
    // on everywhere.
    for shift in [0u32, 1, 63, 127, 255] {
        let x = U256::ONE << shift;
        assert_eq!(x.wrapping_neg(), (!x).wrapping_add(U256::ONE), "{x:?}");
    }
}

// ---------------------------------------------------------------------------
// Exponentiation
// ---------------------------------------------------------------------------

#[test]
fn pow_matches_i128() {
    let mut rng = Rng(0x0246_8ace_1357_9bdf);
    for _ in 0..5000 {
        let base = rng.next_i128() % 1000;
        let exp = (rng.next_u64() % 40) as u32;
        if let Some(v) = base.checked_pow(exp) {
            assert_eq!(
                I256::from(base).checked_pow(exp),
                Some(I256::from(v)),
                "{base}^{exp}"
            );
        }
    }
    assert_eq!(I256::ZERO.pow(0), I256::ONE);
    assert_eq!(I256::MINUS_ONE.pow(255), I256::MINUS_ONE);
    assert_eq!(I256::MINUS_ONE.pow(256), I256::ONE);
    assert_eq!(I256::from(2).pow(254), I256::ONE.wrapping_shl(254));
    assert_eq!(I256::from(2).checked_pow(255), None);
    assert_eq!(I256::from(-2).checked_pow(255), Some(I256::MIN));
    assert_eq!(I256::from(-2).checked_pow(256), None);
    // Saturation follows the sign the true result would have had.
    assert_eq!(I256::from(-3).saturating_pow(255), I256::MIN);
    assert_eq!(I256::from(-3).saturating_pow(254), I256::MAX);
    assert_eq!(I256::from(3).saturating_pow(255), I256::MAX);
}

// ---------------------------------------------------------------------------
// SignedMath
// ---------------------------------------------------------------------------

#[test]
fn min_max_and_clamp_match_ord() {
    let vals = boundary_values();
    for &a in &vals {
        for &b in &vals {
            assert_eq!(a.min(b), Ord::min(a, b), "min({a:?}, {b:?})");
            assert_eq!(a.max(b), Ord::max(a, b), "max({a:?}, {b:?})");
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
    let _ = I256::ZERO.clamp(I256::ONE, I256::MINUS_ONE);
}

#[test]
fn average_rounds_toward_zero_and_never_overflows() {
    let mut rng = Rng(0x5555_6666_7777_8888);
    for _ in 0..10_000 {
        // Halved operands, so the i128 sum is exact and is the specification.
        let a = rng.next_i128() / 2;
        let b = rng.next_i128() / 2;
        assert_eq!(
            I256::from(a).average(I256::from(b)),
            I256::from((a + b) / 2),
            "average({a}, {b})"
        );
    }

    // The cases the bit trick exists for: sums that do not fit.
    assert_eq!(I256::MAX.average(I256::MAX), I256::MAX);
    assert_eq!(I256::MIN.average(I256::MIN), I256::MIN);
    assert_eq!(I256::MAX.average(I256::MIN), I256::ZERO);
    assert_eq!(I256::MIN.average(I256::MAX), I256::ZERO);

    // Rounding toward zero, in both directions, on an odd sum.
    assert_eq!(I256::from(-3).average(I256::ZERO), I256::MINUS_ONE);
    assert_eq!(I256::from(3).average(I256::ZERO), I256::ONE);
    assert_eq!(I256::from(-4).average(I256::ZERO), I256::from(-2));
    assert_eq!(I256::from(-1).average(I256::from(-2)), I256::MINUS_ONE);
    // Which is where it differs from the unsigned version's plain floor.
    assert_eq!(U256::from(3).average(U256::ONE), U256::from(2));
}

#[test]
fn signed_mul_div_matches_the_wide_oracle() {
    let mut rng = Rng(0xdead_beef_feed_face);
    let mut sampled = 0usize;
    for _ in 0..8000 {
        let x = rng.next_i256(2);
        let y = rng.next_i256(2);
        let d = rng.next_i256(2);
        if d.is_zero() {
            continue;
        }

        let negative = (x.is_negative() != y.is_negative()) != d.is_negative();
        let mag_q = (wide(x.unsigned_abs()) * wide(y.unsigned_abs()))
            / wide(d.unsigned_abs());
        let mag_r = (wide(x.unsigned_abs()) * wide(y.unsigned_abs()))
            % wide(d.unsigned_abs());

        let limit = if negative {
            wide(U256::ONE << 255u32)
        } else {
            wide(I256::MAX.cast_unsigned())
        };
        let expect = if mag_q > limit {
            None
        } else {
            let m = narrow(mag_q).unwrap();
            Some(if negative { m.wrapping_neg() } else { m }.cast_signed())
        };
        assert_eq!(
            I256::checked_mul_div(x, y, d),
            expect,
            "mul_div({x:?}, {y:?}, {d:?})"
        );

        // Rounding: away from zero exactly when the direction says so.
        if let Some(q) = expect {
            for rounding in ROUNDINGS {
                let away = !mag_r.is_zero()
                    && rounding.rounds_away_from_zero(negative);
                let want = if away {
                    if negative {
                        q.checked_sub(I256::ONE)
                    } else {
                        q.checked_add(I256::ONE)
                    }
                } else {
                    Some(q)
                };
                assert_eq!(
                    I256::checked_mul_div_rounding(x, y, d, rounding),
                    want,
                    "mul_div({x:?}, {y:?}, {d:?}) {rounding:?}"
                );
            }
        }
        sampled += 1;
    }
    assert!(sampled > 5000);
}

#[test]
fn signed_mul_div_truncates_and_rounds_by_direction() {
    let (seven, two) = (I256::from(7), I256::from(2));
    assert_eq!(I256::mul_div(seven, I256::ONE, two), I256::from(3));
    assert_eq!(I256::mul_div(-seven, I256::ONE, two), I256::from(-3));

    // -7/2 is -3.5: floor is -4, ceil is -3, trunc is -3, expand is -4.
    for (rounding, want) in [
        (Rounding::Floor, -4),
        (Rounding::Ceil, -3),
        (Rounding::Trunc, -3),
        (Rounding::Expand, -4),
    ] {
        assert_eq!(
            I256::mul_div_rounding(-seven, I256::ONE, two, rounding),
            I256::from(want),
            "{rounding:?}"
        );
    }
    // 7/2 is 3.5: floor 3, ceil 4, trunc 3, expand 4.
    for (rounding, want) in [
        (Rounding::Floor, 3),
        (Rounding::Ceil, 4),
        (Rounding::Trunc, 3),
        (Rounding::Expand, 4),
    ] {
        assert_eq!(
            I256::mul_div_rounding(seven, I256::ONE, two, rounding),
            I256::from(want),
            "{rounding:?}"
        );
    }

    // A magnitude-zero inexact quotient still knows which side it came from.
    assert_eq!(
        I256::mul_div_rounding(
            I256::ONE,
            I256::ONE,
            I256::from(-3),
            Rounding::Floor
        ),
        I256::MINUS_ONE
    );
    assert_eq!(
        I256::mul_div_rounding(
            I256::ONE,
            I256::ONE,
            I256::from(-3),
            Rounding::Ceil
        ),
        I256::ZERO
    );

    // The intermediate really is 512 bits wide.
    assert_eq!(I256::mul_div(I256::MAX, I256::MIN, I256::MIN), I256::MAX);
    assert_eq!(I256::checked_mul_div(I256::MIN, I256::MIN, I256::ONE), None);
    assert_eq!(
        I256::checked_mul_div(I256::MIN, I256::ONE, I256::ONE),
        Some(I256::MIN)
    );
    assert_eq!(
        I256::checked_mul_div(I256::ONE, I256::ONE, I256::ZERO),
        None
    );
}

#[test]
#[should_panic(expected = "mul_div quotient out of range for I256")]
fn signed_mul_div_overflow_panics() {
    let _ = I256::mul_div(I256::MIN, I256::MINUS_ONE, I256::ONE);
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

#[test]
fn display_matches_i128() {
    let mut rng = Rng(0x2718_2818_2845_9045);
    let mut vals: Vec<i128> = vec![0, 1, -1, 10, -10, i128::MAX, i128::MIN];
    for _ in 0..3000 {
        vals.push(rng.next_i128());
    }
    for n in vals {
        assert_eq!(format!("{}", I256::from(n)), format!("{n}"), "{n}");
        assert_eq!(format!("{:+}", I256::from(n)), format!("{n:+}"), "{n}");
    }

    // The 256-bit extremes, which no primitive can render.
    assert_eq!(
        format!("{}", I256::MAX),
        "57896044618658097711785492504343953926634992332820282019728792003956564819967"
    );
    assert_eq!(
        format!("{}", I256::MIN),
        "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
    );
}

#[test]
fn from_str_round_trips_display() {
    let mut rng = Rng(0x1618_0339_8874_9895);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..500 {
            vals.push(rng.next_i256(limbs));
        }
    }
    for n in vals {
        let s = format!("{n}");
        assert_eq!(s.parse::<I256>(), Ok(n), "{s}");
    }
    // A leading `+` is accepted and means nothing.
    assert_eq!("+42".parse::<I256>(), Ok(I256::from(42)));
    assert_eq!("-0".parse::<I256>(), Ok(I256::ZERO));
}

#[test]
fn from_str_rejects_bad_input() {
    assert_eq!("".parse::<I256>(), Err(ParseI256Error::Empty));
    assert_eq!("-".parse::<I256>(), Err(ParseI256Error::InvalidDigit));
    assert_eq!("+".parse::<I256>(), Err(ParseI256Error::InvalidDigit));
    assert_eq!("12a".parse::<I256>(), Err(ParseI256Error::InvalidDigit));
    assert_eq!("1-2".parse::<I256>(), Err(ParseI256Error::InvalidDigit));
    assert_eq!(" 1".parse::<I256>(), Err(ParseI256Error::InvalidDigit));

    // The asymmetric range: MIN parses, MIN - 1 and MAX + 1 do not.
    assert_eq!(format!("{}", I256::MIN).parse::<I256>(), Ok(I256::MIN));
    assert_eq!(format!("{}", I256::MAX).parse::<I256>(), Ok(I256::MAX));
    let max_plus_one = format!("{}", I256::MAX.cast_unsigned() + U256::ONE);
    assert_eq!(max_plus_one.parse::<I256>(), Err(ParseI256Error::Overflow));
    let min_minus_one = format!("-{}", I256::MIN.unsigned_abs() + U256::ONE);
    assert_eq!(min_minus_one.parse::<I256>(), Err(ParseI256Error::Overflow));
    // But the same magnitude is fine on the negative side only.
    assert_eq!(
        format!("-{}", I256::MIN.unsigned_abs()).parse::<I256>(),
        Ok(I256::MIN)
    );
    assert_eq!(
        format!("{}", I256::MIN.unsigned_abs()).parse::<I256>(),
        Err(ParseI256Error::Overflow)
    );
}

#[test]
fn hex_shows_the_two_complement_bits_and_debug_shows_the_sign() {
    let mut rng = Rng(0x4142_1356_2373_0951);
    let mut vals = boundary_values();
    for _ in 0..500 {
        vals.push(rng.next_i256(4));
    }
    for n in vals {
        // LowerHex/UpperHex print the raw bits, as they do for the
        // primitives: `format!("{:x}", -1i8)` is "ff", not "-1".
        assert_eq!(
            format!("{n:x}"),
            format!("{:x}", n.cast_unsigned()),
            "{n:?}"
        );
        assert_eq!(format!("{n:X}"), format!("{n:x}").to_uppercase());
        assert_eq!(format!("{n:#x}"), format!("0x{n:x}"));

        // Debug is sign-magnitude, so a diagnostic reads.
        let expect = if n.is_negative() {
            format!("-0x{:x}", n.unsigned_abs())
        } else {
            format!("0x{:x}", n.unsigned_abs())
        };
        assert_eq!(format!("{n:?}"), expect);
    }

    assert_eq!(format!("{:x}", I256::MINUS_ONE), "f".repeat(64));
    assert_eq!(format!("{:?}", I256::MINUS_ONE), "-0x1");
    assert_eq!(format!("{:?}", I256::from(-42)), "-0x2a");
    assert_eq!(format!("{:?}", I256::from(42)), "0x2a");
    assert_eq!(format!("{:?}", I256::ZERO), "0x0");
}

#[test]
fn formatting_honours_width_and_padding_flags() {
    let n = I256::from(-31);
    assert_eq!(format!("{n:>6}"), "   -31");
    assert_eq!(format!("{n:06}"), "-00031");
    assert_eq!(format!("{n:*^7}"), "**-31**");
    assert_eq!(format!("{n:+}"), "-31");
    assert_eq!(format!("{:+}", I256::from(31)), "+31");
    // Hex pads over the raw bits, which are 64 digits wide for a negative.
    assert_eq!(format!("{:064x}", I256::from(31)), format!("{:062}1f", 0));
    assert_eq!(format!("{:064x}", I256::MINUS_ONE).len(), 64);
}

// ---------------------------------------------------------------------------
// Byte and XDR representation
// ---------------------------------------------------------------------------

#[test]
fn be_bytes_round_trip_and_are_two_complement() {
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..500 {
            vals.push(rng.next_i256(limbs));
        }
    }
    for n in vals {
        let bytes = n.to_be_bytes();
        assert_eq!(I256::from_be_bytes(bytes), n);
        // Same 32 bytes the unsigned view produces -- that is what "two's
        // complement on the wire" means.
        assert_eq!(bytes, n.cast_unsigned().to_be_bytes(), "{n:?}");
    }

    assert_eq!(I256::MINUS_ONE.to_be_bytes(), [0xffu8; 32]);
    assert_eq!(I256::ZERO.to_be_bytes(), [0u8; 32]);
    let mut min = [0u8; 32];
    min[0] = 0x80;
    assert_eq!(I256::MIN.to_be_bytes(), min);
}

#[test]
fn xdr_round_trips() {
    let mut rng = Rng(0xfedc_ba98_7654_3210);
    let mut vals = boundary_values();
    for limbs in 1..=4 {
        for _ in 0..500 {
            vals.push(rng.next_i256(limbs));
        }
    }
    for n in vals {
        let bytes = xdrs::txs::encode(&n).expect("encode");
        assert_eq!(bytes.len(), 32, "{n:?}");
        let decoded: I256 = xdrs::txs::decode(&bytes).expect("decode");
        assert_eq!(decoded, n, "{n:?}");
    }
}

/// Pins the encoding, which is wire-visible: 32 big-endian two's complement
/// bytes, the same convention Ethereum's `int256` uses.
#[test]
fn xdr_encoding_is_big_endian_two_complement() {
    let n = I256::from(-2);
    let bytes = xdrs::txs::encode(&n).expect("encode");
    let mut expected = [0xffu8; 32];
    expected[31] = 0xfe;
    assert_eq!(bytes, expected);

    let n = I256::from(0x0102_0304_0506_0708);
    let bytes = xdrs::txs::encode(&n).expect("encode");
    let mut expected = [0u8; 32];
    expected[24..].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(bytes, expected);

    // MIN is the sign bit alone.
    let mut min = [0u8; 32];
    min[0] = 0x80;
    assert_eq!(xdrs::txs::encode(&I256::MIN).expect("encode"), min);
}

// ---------------------------------------------------------------------------
// Ergonomics
// ---------------------------------------------------------------------------

/// The call shapes a port from Solidity writes by reflex. As with the
/// unsigned suite, the point is that this file *compiles*.
#[test]
fn integer_literals_and_operators_need_no_annotations() {
    const FEE: I256 = I256::from(-1000);
    const SCALE: I256 = I256::from_i64(1_000_000);

    assert_eq!(I256::from(-1000), FEE);
    assert_eq!(SCALE, I256::from(1_000_000));

    let a = I256::from(-5);
    assert_eq!(a + I256::ONE, I256::from(-4));
    assert_eq!(a - I256::ONE, I256::from(-6));
    assert_eq!(a * I256::from(3), I256::from(-15));
    assert_eq!(a / I256::from(2), I256::from(-2));
    assert_eq!(a % I256::from(2), I256::MINUS_ONE);
    assert_eq!(-a, I256::from(5));
    // Bare literal shift amounts resolve to `i32`, which the full shift
    // family covers.
    assert_eq!(a << 2, I256::from(-20));
    assert_eq!(a >> 1, I256::from(-3));
    assert_eq!(a << 2u8, a << 2usize);
}

/// The signed surface is `const fn` throughout, which contracts lean on.
#[test]
fn the_signed_surface_is_const_evaluable() {
    const A: I256 = I256::from(-1_000_000_000_000_000_000);
    const B: I256 = I256::from(3);
    const NEG: I256 = A.wrapping_neg();
    const ABS: U256 = A.unsigned_abs();
    const AVG: I256 = A.average(B);
    const QUOT: I256 = I256::mul_div(A, B, I256::from(7));
    const SHIFTED: I256 = A.wrapping_shr(4);
    const CMP: core::cmp::Ordering = A.const_cmp(B);

    assert_eq!(NEG, -A);
    assert_eq!(ABS, U256::from(1_000_000_000_000_000_000));
    assert_eq!(AVG, A.average(B));
    assert_eq!(QUOT, I256::mul_div(A, B, I256::from(7)));
    assert_eq!(SHIFTED, A.div_euclid(I256::from(16)));
    assert_eq!(CMP, core::cmp::Ordering::Less);
}
