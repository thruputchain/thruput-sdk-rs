//! Edge-case parity with OpenZeppelin's `Math.sol` and `SignedMath.sol`.
//!
//! The other suites check that this crate computes the right *numbers*, against
//! `u128`/`i128` and a wide oracle. This one checks something narrower and
//! easier to get wrong: that each library function answers the way the Solidity
//! one does on the inputs nobody sweeps -- zero moduli, zero divisors, `0^0`,
//! degenerate primes, and the boundary between "returns a sentinel" and
//! "reverts".
//!
//! Every expectation below is traced from the Solidity source rather than
//! guessed, and the trace is in the comment. These are the cases where a port
//! silently diverges: the arithmetic agrees on ordinary inputs, so nothing else
//! in the suite would notice.
//!
//! # Scope
//!
//! This file is about *library functions*. It says nothing about the operators
//! (`+`, `-`, `*`, `/`, `%`), whose overflow behaviour follows Rust's
//! primitives and the build's `overflow-checks` flag -- deliberately, and
//! documented in `signed`'s module docs. Solidity and Rust genuinely differ
//! there (`type(int).min % -1` is 0 in Solidity and a panic in Rust), and that
//! difference is a language-level one, not something a library function
//! decides.

use thruput_u256::{I256, Rounding, U256};

/// `Math.tryDiv` / `Math.tryMod` return `(false, 0)` for a zero divisor rather
/// than reverting -- the `try*` family's whole purpose. `checked_*` is the
/// same contract with `Option` in place of the bool.
#[test]
fn try_family_reports_rather_than_reverting() {
    assert_eq!(U256::from(5).checked_div(U256::ZERO), None);
    assert_eq!(U256::from(5).checked_rem(U256::ZERO), None);
    assert_eq!(U256::ZERO.checked_div(U256::ZERO), None);

    // tryAdd/trySub/tryMul, at the boundaries where they flip.
    assert_eq!(U256::MAX.checked_add(U256::ONE), None);
    assert_eq!(U256::MAX.checked_add(U256::ZERO), Some(U256::MAX));
    assert_eq!(U256::ZERO.checked_sub(U256::ONE), None);
    assert_eq!(U256::MAX.checked_mul(U256::from(2)), None);

    // `saturatingSub` bottoms out at zero, `saturatingAdd`/`Mul` at MAX.
    assert_eq!(U256::ZERO.saturating_sub(U256::ONE), U256::ZERO);
    assert_eq!(U256::MAX.saturating_add(U256::ONE), U256::MAX);
    assert_eq!(U256::MAX.saturating_mul(U256::from(2)), U256::MAX);
}

/// `Math.ceilDiv` checks `b == 0` *before* its `a > 0` shortcut, so `0 / 0`
/// reverts rather than answering zero. Getting that order wrong is the obvious
/// way to write this function and silently wrong.
#[test]
fn ceil_div_rejects_a_zero_divisor_even_for_a_zero_dividend() {
    assert_eq!(U256::ZERO.checked_ceil_div(U256::ZERO), None);
    assert_eq!(U256::ONE.checked_ceil_div(U256::ZERO), None);
    assert_eq!(U256::ZERO.checked_ceil_div(U256::ONE), Some(U256::ZERO));
    // Rounds toward infinity, so any remainder costs a whole unit.
    assert_eq!(U256::ONE.ceil_div(U256::from(2)), U256::ONE);
    assert_eq!(U256::from(4).ceil_div(U256::from(2)), U256::from(2));
    assert_eq!(U256::from(5).ceil_div(U256::from(2)), U256::from(3));
}

/// `Math.mulDiv` panics on a zero denominator and on a quotient that will not
/// fit; the rounding variant adds one afterwards, so it can leave the range a
/// floor quotient just fit.
#[test]
fn mul_div_boundaries() {
    assert_eq!(
        U256::checked_mul_div(U256::ONE, U256::ONE, U256::ZERO),
        None
    );
    assert_eq!(
        U256::checked_mul_div(U256::ZERO, U256::ZERO, U256::ONE),
        Some(U256::ZERO)
    );
    // `x * y` far past 256 bits, quotient inside it: the case the 512-bit
    // intermediate exists for.
    assert_eq!(
        U256::checked_mul_div(U256::MAX, U256::MAX, U256::MAX),
        Some(U256::MAX)
    );
    // Quotient past 256 bits.
    assert_eq!(U256::checked_mul_div(U256::MAX, U256::MAX, U256::ONE), None);

    // OZ computes `mulDiv(...) + toUint(...)`, and that addition is checked,
    // so a rounding step out of range reverts rather than wrapping.
    assert_eq!(
        U256::checked_mul_div_rounding(
            U256::MAX,
            U256::MAX,
            U256::MAX - U256::ONE,
            Rounding::Ceil
        ),
        None
    );
}

/// `Math.mulShr`'s rounding term is `mulmod(x, y, 1 << n) > 0`. At `n == 0`
/// that modulus is 1, so the term is always zero: a zero-bit shift drops
/// nothing and must never round up, whatever the direction.
#[test]
fn mul_shr_never_rounds_at_a_zero_shift() {
    for rounding in [
        Rounding::Floor,
        Rounding::Ceil,
        Rounding::Trunc,
        Rounding::Expand,
    ] {
        assert_eq!(
            U256::checked_mul_shr_rounding(
                U256::from(7),
                U256::from(9),
                0,
                rounding
            ),
            Some(U256::from(63)),
            "{rounding:?}"
        );
    }
    // And at a shift that does drop bits, only the up directions move.
    assert_eq!(
        U256::mul_shr_rounding(U256::from(7), U256::ONE, 1, Rounding::Floor),
        U256::from(3)
    );
    assert_eq!(
        U256::mul_shr_rounding(U256::from(7), U256::ONE, 1, Rounding::Ceil),
        U256::from(4)
    );
}

/// `Math.invMod` returns zero rather than reverting for every input without an
/// inverse, including both degenerate moduli.
///
/// Traced: `n == 0` returns 0 outright. For `n == 1`, `a % 1` is 0 so the loop
/// never runs, `gcd` stays 1, and the returned coefficient is its initial 0.
/// For a non-coprime pair the loop ends with `gcd != 1`, which returns 0.
#[test]
fn inv_mod_answers_zero_where_no_inverse_exists() {
    assert_eq!(U256::inv_mod(U256::from(3), U256::ZERO), U256::ZERO);
    assert_eq!(U256::inv_mod(U256::from(3), U256::ONE), U256::ZERO);
    assert_eq!(U256::inv_mod(U256::ZERO, U256::from(7)), U256::ZERO);
    // 2 and 4 share a factor.
    assert_eq!(U256::inv_mod(U256::from(2), U256::from(4)), U256::ZERO);
    // And a real inverse still comes back.
    assert_eq!(U256::inv_mod(U256::from(3), U256::from(7)), U256::from(5));
}

/// `Math.invModPrime` is `modExp(a, p - 2, p)` inside an `unchecked` block, so
/// its degenerate cases are whatever that subtraction wraps to.
///
/// Traced: `p == 1` wraps to `2^256 - 1`, and `modExp(_, _, 1)` is 0. `p == 0`
/// wraps to `2^256 - 2`, and `modExp` with a zero modulus reverts. Note this
/// is *not* the same shape as `invMod`, which is total -- so the two disagree
/// on a zero modulus, and that is faithful.
#[test]
fn inv_mod_prime_degenerate_moduli_follow_the_wrapping_subtraction() {
    assert_eq!(U256::inv_mod_prime(U256::from(3), U256::ONE), U256::ZERO);
    // Where p really is prime, it agrees with the general routine.
    let p = U256::from(7);
    assert_eq!(
        U256::inv_mod_prime(U256::from(3), p),
        U256::inv_mod(U256::from(3), p)
    );
}

#[test]
#[should_panic(expected = "attempt to divide by zero")]
fn inv_mod_prime_reverts_on_a_zero_modulus_where_inv_mod_returns_zero() {
    // The asymmetry is inherited from Solidity: `invMod` guards `n == 0`,
    // `invModPrime` does not and reaches `modExp`'s guard instead.
    assert_eq!(U256::inv_mod(U256::from(3), U256::ZERO), U256::ZERO);
    let _ = U256::inv_mod_prime(U256::from(3), U256::ZERO);
}

/// `Math.modExp`'s edges, which are OpenZeppelin's and not quite the
/// precompile's.
///
/// A zero modulus is the one that differs. The EIP-198 precompile treats it as
/// defined: go-ethereum's `Run` short-circuits `mod.BitLen() == 0` and returns
/// a zero-filled buffer, with the comment "Modulo 0 is undefined, return
/// zero". Solidity never sees that, because `tryModExp` guards `m == 0` and
/// returns `(false, 0)` *before* the `staticcall`, which `modExp` turns into a
/// revert. So the error is OpenZeppelin's addition, not the precompile's
/// behaviour, and matching OpenZeppelin is the right call here -- a port is
/// written against the library, not the precompile -- but the two are not the
/// same rule.
///
/// The rest is genuinely EIP-198: `0^0` is one, and everything modulo one is
/// zero.
#[test]
fn mod_exp_matches_openzeppelin_at_the_edges() {
    assert_eq!(
        U256::checked_mod_exp(U256::from(2), U256::from(3), U256::ZERO),
        None
    );
    // Everything is congruent to zero modulo one, `0^0` included.
    assert_eq!(
        U256::checked_mod_exp(U256::from(2), U256::from(3), U256::ONE),
        Some(U256::ZERO)
    );
    assert_eq!(
        U256::checked_mod_exp(U256::ZERO, U256::ZERO, U256::ONE),
        Some(U256::ZERO)
    );
    // `b^0 == 1` for every base, zero included.
    assert_eq!(
        U256::checked_mod_exp(U256::ZERO, U256::ZERO, U256::from(7)),
        Some(U256::ONE)
    );
    assert_eq!(
        U256::checked_mod_exp(U256::from(5), U256::ZERO, U256::from(7)),
        Some(U256::ONE)
    );
    // `0^e == 0` for a nonzero exponent.
    assert_eq!(
        U256::checked_mod_exp(U256::ZERO, U256::from(5), U256::from(7)),
        Some(U256::ZERO)
    );
}

/// `Math.clz` is `x == 0 ? 256 : 255 - log2(x)`. Ours is
/// `BITS - bit_len(x)`; the two agree because `bit_len == log2 + 1` above
/// zero, and this pins that rather than leaving it to the reader.
#[test]
fn leading_zeros_matches_the_clz_formula() {
    assert_eq!(U256::ZERO.leading_zeros(), 256);
    for shift in 0..256u32 {
        let x = U256::ONE << shift;
        assert_eq!(x.leading_zeros(), 255 - x.log2(), "1 << {shift}");
        let y = U256::MAX >> shift;
        assert_eq!(y.leading_zeros(), 255 - y.log2(), "MAX >> {shift}");
    }
    assert_eq!(U256::MAX.leading_zeros(), 0);
}

/// The `log*` and `sqrt` families all answer zero for zero, and their rounding
/// variants must not turn that into one -- `10^0` and `2^0` are *above* zero,
/// so the `<` test is false.
#[test]
fn logs_and_sqrt_of_zero_never_round_up() {
    for rounding in [
        Rounding::Floor,
        Rounding::Ceil,
        Rounding::Trunc,
        Rounding::Expand,
    ] {
        assert_eq!(U256::ZERO.log2_rounding(rounding), 0, "{rounding:?}");
        assert_eq!(U256::ZERO.log10_rounding(rounding), 0, "{rounding:?}");
        assert_eq!(U256::ZERO.log256_rounding(rounding), 0, "{rounding:?}");
        assert_eq!(
            U256::ZERO.sqrt_rounding(rounding),
            U256::ZERO,
            "{rounding:?}"
        );
        // One is exact in all four, so it never moves either.
        assert_eq!(U256::ONE.log2_rounding(rounding), 0, "{rounding:?}");
        assert_eq!(U256::ONE.log10_rounding(rounding), 0, "{rounding:?}");
        assert_eq!(
            U256::ONE.sqrt_rounding(rounding),
            U256::ONE,
            "{rounding:?}"
        );
    }
    // And the first inexact value in each does move, under the up directions.
    assert_eq!(U256::from(2).log2_rounding(Rounding::Ceil), 1);
    assert_eq!(U256::from(3).log2_rounding(Rounding::Ceil), 2);
    assert_eq!(U256::from(2).log10_rounding(Rounding::Ceil), 1);
    assert_eq!(U256::from(2).sqrt_rounding(Rounding::Ceil), U256::from(2));
}

/// `Math.average` rounds toward zero; `SignedMath.average` does too, which is
/// the one place the signed version is not just the unsigned one with a sign.
#[test]
fn average_rounds_toward_zero_on_both_types() {
    // Unsigned: 1.5 -> 1.
    assert_eq!(U256::ONE.average(U256::from(2)), U256::ONE);
    assert_eq!(U256::from(2).average(U256::from(4)), U256::from(3));
    // Signed, both directions across zero.
    assert_eq!(I256::from(-1).average(I256::from(-2)), I256::from(-1));
    assert_eq!(I256::from(1).average(I256::from(2)), I256::from(1));
    assert_eq!(I256::from(-3).average(I256::ZERO), I256::from(-1));
    assert_eq!(I256::from(3).average(I256::ZERO), I256::ONE);
    // A floor average would have said -2 for the third of those; that is the
    // whole content of the correction term.
    assert_ne!(I256::from(-3).average(I256::ZERO), I256::from(-2));
}

/// `SignedMath.abs` returns an unsigned value precisely so `type(int).min` has
/// somewhere to go. Ours is `unsigned_abs`, and it is total for the same
/// reason.
#[test]
fn signed_abs_is_total_at_min() {
    assert_eq!(I256::MIN.unsigned_abs(), U256::ONE << 255u32);
    assert_eq!(I256::MAX.unsigned_abs(), I256::MAX.cast_unsigned());
    assert_eq!(I256::MINUS_ONE.unsigned_abs(), U256::ONE);
    assert_eq!(I256::ZERO.unsigned_abs(), U256::ZERO);
    // The checked signed form is the one that cannot represent it.
    assert_eq!(I256::MIN.checked_abs(), None);
}

/// `SafeCast.sol` is a table of narrowing casts that revert out of range;
/// `TryFrom` is the same contract with `Result`. This pins that the table has
/// no holes at the widths Rust has primitives for, in both directions.
///
/// `SafeCast`'s off-width members (`toUint248`, `toUint240`, …) have no Rust
/// counterpart to convert into; a port needing those checks the bound against
/// a `U256` mask itself. `usize`/`isize` are deliberately absent too: they are
/// 32-bit on the guest and 64-bit on the host, so a conversion that succeeded
/// in a test could fail on chain.
#[test]
fn safe_cast_table_has_no_holes() {
    // Unsigned narrowing, at each width's boundary.
    assert_eq!(u8::try_from(U256::from(255)), Ok(255u8));
    assert!(u8::try_from(U256::from(256)).is_err());
    assert_eq!(u16::try_from(U256::from(65_535)), Ok(65_535u16));
    assert!(u16::try_from(U256::from(65_536)).is_err());
    assert_eq!(u32::try_from(U256::from(u32::MAX as u128)), Ok(u32::MAX));
    assert!(u32::try_from(U256::from(u32::MAX as u128 + 1)).is_err());
    assert_eq!(u64::try_from(U256::from(u64::MAX as u128)), Ok(u64::MAX));
    assert_eq!(u128::try_from(U256::from(u128::MAX)), Ok(u128::MAX));
    assert!(u128::try_from(U256::from(u128::MAX) + U256::ONE).is_err());

    // Signed narrowing, both ends -- the negative side reaches one further,
    // which is the case a magnitude-based bound gets wrong.
    assert_eq!(i8::try_from(I256::from(127)), Ok(127i8));
    assert_eq!(i8::try_from(I256::from(-128)), Ok(-128i8));
    assert!(i8::try_from(I256::from(128)).is_err());
    assert!(i8::try_from(I256::from(-129)).is_err());
    assert_eq!(i16::try_from(I256::from(-32_768)), Ok(-32_768i16));
    assert!(i16::try_from(I256::from(-32_769)).is_err());
    assert_eq!(i128::try_from(I256::from(i128::MIN)), Ok(i128::MIN));

    // Signed to unsigned refuses anything negative, at every width.
    assert!(u8::try_from(I256::MINUS_ONE).is_err());
    assert!(u16::try_from(I256::MINUS_ONE).is_err());
    assert!(u128::try_from(I256::MINUS_ONE).is_err());
    assert_eq!(u8::try_from(I256::from(255)), Ok(255u8));

    // And the two 256-bit directions, which is `toInt256`/`toUint256`.
    assert_eq!(I256::try_from(U256::ZERO), Ok(I256::ZERO));
    assert!(I256::try_from(U256::MAX).is_err());
    assert_eq!(U256::try_from(I256::MAX), Ok(I256::MAX.cast_unsigned()));
    assert!(U256::try_from(I256::MIN).is_err());
}

/// Every narrowing conversion must agree with doing it one step at a time
/// through the primitives, across a sweep -- the bound is easy to write
/// off-by-one.
#[test]
fn narrowing_matches_the_primitive_chain() {
    let mut x = 1u128;
    let mut probes = vec![0u128, 1, 2, 127, 128, 255, 256, 32_767, 65_535];
    while x < u128::MAX / 3 {
        probes.push(x);
        x = x.wrapping_mul(3).wrapping_add(1);
    }
    for v in probes {
        let u = U256::from(v);
        assert_eq!(u8::try_from(u).ok(), u8::try_from(v).ok(), "{v}");
        assert_eq!(u16::try_from(u).ok(), u16::try_from(v).ok(), "{v}");
        assert_eq!(u32::try_from(u).ok(), u32::try_from(v).ok(), "{v}");

        let signed = i128::try_from(v).unwrap_or(i128::MAX);
        let i = I256::from(signed);
        assert_eq!(
            i8::try_from(i).ok(),
            i8::try_from(signed).ok(),
            "{signed}"
        );
        assert_eq!(
            i16::try_from(i).ok(),
            i16::try_from(signed).ok(),
            "{signed}"
        );
        let neg = I256::from(-signed);
        assert_eq!(i8::try_from(neg).ok(), i8::try_from(-signed).ok());
        assert_eq!(i16::try_from(neg).ok(), i16::try_from(-signed).ok());
    }
}

/// `Math.unsignedRoundsUp` is `uint8(rounding) % 2 == 1`, which pins the
/// discriminant order as much as the predicate.
#[test]
fn rounding_discriminants_match_solidity() {
    assert!(!Rounding::Floor.unsigned_rounds_up());
    assert!(Rounding::Ceil.unsigned_rounds_up());
    assert!(!Rounding::Trunc.unsigned_rounds_up());
    assert!(Rounding::Expand.unsigned_rounds_up());
    // Declaration order is Floor, Ceil, Trunc, Expand -- the odd ones round up.
    assert_eq!(Rounding::Floor as u8, 0);
    assert_eq!(Rounding::Ceil as u8, 1);
    assert_eq!(Rounding::Trunc as u8, 2);
    assert_eq!(Rounding::Expand as u8, 3);
}
