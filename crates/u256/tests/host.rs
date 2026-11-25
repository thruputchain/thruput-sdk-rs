//! Division, formatting and square roots with no runtime: a host program,
//! such as the CLI printing a contract's `U256` return value, has no native
//! executor and runs no transaction.

use thruput_u256::{
    I256, U256, checked_mul_div_i128, checked_mul_div_u128, mul_div_i128,
    mul_div_u128,
};

#[test]
fn division_needs_no_transaction() {
    let ten = U256::from(10);
    assert_eq!(U256::from(1_234_567) / ten, U256::from(123_456));
    assert_eq!(U256::from(1_234_567) % ten, U256::from(7));
    assert_eq!(U256::MAX.div_rem(U256::MAX), (U256::ONE, U256::ZERO));
    assert_eq!(
        U256::MAX.div_rem(U256::ONE << 128),
        (U256::from(u128::MAX), U256::from(u128::MAX))
    );
    assert_eq!(I256::from(-7) / I256::from(2), I256::from(-3));
    assert_eq!(U256::MAX.checked_div(U256::ZERO), None);
}

#[test]
fn mul_div_needs_no_transaction() {
    // A 512-bit product, back inside 256 bits.
    assert_eq!(U256::mul_div(U256::MAX, U256::MAX, U256::MAX), U256::MAX);
    assert_eq!(U256::checked_mul_div(U256::MAX, U256::MAX, U256::ONE), None);
    assert_eq!(
        U256::checked_mul_mod(U256::MAX, U256::MAX, U256::from(7)),
        Some(U256::from(1))
    );
    assert_eq!(mul_div_u128(u128::MAX, u128::MAX, u128::MAX), u128::MAX);
    assert_eq!(checked_mul_div_u128(u128::MAX, 2, 1), None);
    assert_eq!(mul_div_i128(i128::MAX, -3, 3), -i128::MAX);
    assert_eq!(checked_mul_div_i128(i128::MIN, -1, 1), None);
}

#[test]
fn square_roots_need_no_transaction() {
    assert_eq!(U256::ZERO.integer_sqrt(), U256::ZERO);
    assert_eq!(U256::from(99).integer_sqrt(), U256::from(9));
    assert_eq!(U256::from(100).integer_sqrt(), U256::from(10));
    assert_eq!(U256::MAX.integer_sqrt(), U256::from(u128::MAX));
}

#[test]
fn decimal_formatting_needs_no_transaction() {
    assert_eq!(
        U256::MAX.to_string(),
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
    assert_eq!(
        I256::MIN.to_string(),
        "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
    );
    assert_eq!(
        "340282366920938463463374607431768211456".parse(),
        Ok(U256::ONE << 128)
    );
}
