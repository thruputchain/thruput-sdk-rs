//! Call-shape regressions.
//!
//! Every assertion here is incidental; the point of the file is that it
//! *compiles*. These are the shapes a port from `uint`/`ruint` writes by
//! reflex, and each one has failed to build at some point:
//!
//! * an unsuffixed integer literal reaching `U256::from`,
//! * `U256::from` in const position,
//! * a shift by `usize` (and by a bare literal, which resolves to `i32` once
//!   more than one `Shl` impl exists).
//!
//! See `docs/` and the `arith.rs` shift impls for the reasoning.

use groundhog_u256::U256;

// `From::from` is not a const fn, so this line only builds against the
// inherent `U256::from`.
const FEE: U256 = U256::from(1000);

#[test]
fn from_accepts_an_unsuffixed_literal() {
    assert_eq!(U256::from(1000), FEE);
}

#[test]
fn from_u32_and_from_u64_take_typed_values_without_widening() {
    let small: u32 = 7;
    let big: u64 = u64::MAX;

    assert_eq!(U256::from_u32(small), U256::from(7));
    assert_eq!(U256::from_u64(big), U256::from(u64::MAX as u128));
}

#[test]
fn from_u32_and_from_u64_are_const() {
    const SMALL: U256 = U256::from_u32(7);
    const BIG: U256 = U256::from_u64(u64::MAX);

    assert_eq!(SMALL, U256::from(7));
    assert_eq!(BIG, U256::from(u64::MAX as u128));
}

#[test]
fn the_from_trait_impls_are_still_reachable() {
    let x: u64 = 5;
    let via_into: U256 = x.into();

    assert_eq!(via_into, U256::from(5));
    assert_eq!(<U256 as From<u32>>::from(5), U256::from(5));

    // UFCS is the only spelling that can reach `From<u128>`: the inherent
    // `U256::from` shadows it in path form, and no unsuffixed literal or
    // `.into()` in the other tests lands on it. If the trait body ever
    // regresses into calling itself, this is the test that stack-overflows.
    assert_eq!(<U256 as From<u128>>::from(1 << 100), U256::ONE << 100u32);
}

#[test]
fn shifts_accept_every_integer_width() {
    let one = U256::ONE;
    let expected = U256::from(1u128 << 112);

    assert_eq!(one << 112, expected);
    assert_eq!(one << 112u8, expected);
    assert_eq!(one << 112u32, expected);
    assert_eq!(one << 112u64, expected);
    assert_eq!(one << 112usize, expected);
    assert_eq!(one << 112i32, expected);
    assert_eq!(one << 112isize, expected);

    assert_eq!(expected >> 112, one);
    assert_eq!(expected >> 112usize, one);
    assert_eq!(expected >> 112i32, one);
}

#[test]
fn shift_assign_accepts_every_integer_width() {
    let mut x = U256::ONE;
    x <<= 8;
    x <<= 8usize;
    x <<= 8i32;
    assert_eq!(x, U256::from(1u128 << 24));

    x >>= 8usize;
    assert_eq!(x, U256::from(1u128 << 16));
}
