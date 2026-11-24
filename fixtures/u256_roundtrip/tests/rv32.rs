//! End-to-end `thruput_sdk::U256` and `thruput_sdk::I256` coverage through a
//! real rv32 artifact.
//!
//! Complements `crates/sdk/tests/u256.rs`, which drives U256 through a
//! host-side mock contract: here the compiled `.rv32a` image runs under the
//! rv32 interpreter, so the guest-side calldata decode, typed return
//! encode, and storage syscalls are the ones a deployed contract executes.
//!
//! Requires the lowered `u256_roundtrip` artifact: `cargo gnaw build
//! u256_roundtrip` writes it.

use thruput_sdk::*;
use u256_roundtrip::U256RoundtripClient;

// Test working directory is the example contract directory.
const RV32_PATH: &str = "../../target/thruput/u256_roundtrip.rv32a";

static NONCE: SHA256Hash = sha256_static!("U256_ROUNDTRIP_TEST_NONCE");

/// A distinct deploy nonce, so the signed test gets its own contract instance
/// and cannot read storage the unsigned one wrote.
static SIGNED_NONCE: SHA256Hash = sha256_static!("I256_ROUNDTRIP_TEST_NONCE");

/// And one more, for the `I256::MIN` case below.
static MIN_NONCE: SHA256Hash = sha256_static!("I256_MIN_ROUNDTRIP_TEST_NONCE");

/// And one for the square roots.
static SQRT_NONCE: SHA256Hash = sha256_static!("U256_SQRT_TEST_NONCE");

/// And one for the multiply-divides.
static MUL_DIV_NONCE: SHA256Hash = sha256_static!("U256_MUL_DIV_TEST_NONCE");

/// A value whose limbs all differ and whose top bit is set, so any limb
/// reordering or truncation in the calldata/return/storage paths shows up.
fn test_value() -> U256 {
    U256::widening_mul_u128(
        0xDEAD_BEEF_0011_2233_4455_6677_8899_AABB_u128,
        0x8000_0000_0000_0000_0000_0000_0000_0001_u128,
    )
}

/// A *negative* value with the same property. Its two's complement has every
/// high bit set, which is what an encoder that zero-extends instead of
/// sign-extending would lose.
fn signed_test_value() -> I256 {
    -I256::from(0x0DEA_DBEE_F001_1223_3445_5667_7889_9AAB_i128)
}

/// U256 into the guest as an entrypoint argument, back out through the
/// typed return, and persisted in `Value<U256>` across a block boundary.
#[test]
fn u256_roundtrips_through_rv32_entrypoint_and_storage() {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let arg = test_value();
    let expected = arg.checked_add(U256::ONE).unwrap();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            assert_eq!(client.add_one_and_store(arg), expected);
            // Read back within the same transaction.
            assert_eq!(client.stored(), expected);
        })
    })
    .assert_success();

    env.advance_block();

    // A fresh transaction in the next block: the stored U256 must survive
    // the commit and read back identically from a fresh storage handle.
    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = get_deploy_address(&NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            assert_eq!(client.stored(), expected);
        })
    })
    .assert_success();
}

/// The same three paths for `I256`. Separate from the unsigned test because
/// the failure it guards is different: a sign-extension bug shows up only on
/// a negative value, and only once it has crossed the ABI.
#[test]
fn i256_roundtrips_through_rv32_entrypoint_and_storage() {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let arg = signed_test_value();
    let expected = arg.wrapping_neg();
    assert!(arg.is_negative() && expected.is_positive());

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&SIGNED_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            assert_eq!(client.negate_and_store(arg), expected);
            assert_eq!(client.stored_signed(), expected);

            // And back the other way, so a negative value is what survives
            // the storage round trip rather than only entering it.
            assert_eq!(client.negate_and_store(expected), arg);
            assert_eq!(client.stored_signed(), arg);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = get_deploy_address(&SIGNED_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            assert_eq!(client.stored_signed(), arg);
        })
    })
    .assert_success();
}

/// `I256::MIN` negating to itself, end to end.
///
/// The other signed test cannot reach this case: it asserts the negation
/// flips sign, which `MIN` by definition does not. That left the exact value
/// the contract's own doc comment cites as its reason for calling
/// `wrapping_neg` covered only on the host, by `crates/u256/tests/signed.rs`.
///
/// The guest is where it counts. Contract profiles build with
/// `overflow-checks` off while the native harness builds with them on, so
/// wrapping is what a deployed contract actually gets and the host cannot
/// observe it. `MIN` is also the extreme case for the ABI: `0x8000...0000`
/// is its own two's complement, all 32 bytes of it.
///
/// What this pins, exactly: that `MIN` survives the calldata decode, the
/// guest-side negation, the typed return, and the storage round trip as that
/// bit pattern and no other. Verified by mutation -- an off-by-one before the
/// negation fails it with `MAX` against `MIN`.
///
/// What it does *not* pin, because `MIN` is a fixed point of negation: that
/// the contract negates at all. A body that returned its argument untouched
/// would pass here. That case belongs to the sibling test above, which
/// catches it -- checked by the same mutation.
#[test]
fn i256_min_negates_to_itself_through_rv32() {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    // Not `-I256::MIN`: that is the overflow this test exists to pin, and
    // the operator would panic on it here (the harness builds checked).
    let arg = I256::MIN;
    assert!(arg.is_negative() && arg.wrapping_neg().is_negative());

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&MIN_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            // Negating in the guest returns `MIN` unchanged...
            assert_eq!(client.negate_and_store(arg), I256::MIN);
            assert_eq!(client.stored_signed(), I256::MIN);

            // ...and stays there, rather than drifting on a second pass.
            assert_eq!(client.negate_and_store(I256::MIN), I256::MIN);
            assert_eq!(client.stored_signed(), I256::MIN);
        })
    })
    .assert_success();

    env.advance_block();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = get_deploy_address(&MIN_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            assert_eq!(client.stored_signed(), I256::MIN);
        })
    })
    .assert_success();
}

/// Values a square root can get wrong: the ends of the range, each side of
/// 2^128 (the largest root is 2^128 - 1), squares and their neighbours at
/// every root length, and values of every bit length with scattered bits.
fn sqrt_values() -> Vec<U256> {
    let mut values = vec![
        U256::ZERO,
        U256::ONE,
        U256::from(2),
        U256::from(3),
        U256::from(4),
        U256::from(u128::MAX) - U256::ONE,
        U256::from(u128::MAX),
        U256::from(u128::MAX) + U256::ONE,
        U256::from(u128::MAX) + U256::from(2),
        U256::MAX - U256::ONE,
        U256::MAX,
        test_value(),
    ];
    // A root of every length from 1 to 128 bits: its square, the value below
    // it, and the largest value with that root.
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for bits in 1..=128_u32 {
        let top = 1_u128 << (bits - 1);
        let noise = (u128::from(next()) << 64) | u128::from(next());
        let root = U256::from(top | (noise & (top - 1)));
        let square = root * root;
        values.push(square);
        values.push(square - U256::ONE);
        values.push(square + root + root);
    }
    // And a value of every bit length from 1 to 256.
    for bits in 1..=256_u32 {
        let noise = U256::from_limbs(core::array::from_fn(|_| next() as u32));
        let top = U256::ONE << (bits - 1);
        values.push(top | (noise & (top - U256::ONE)));
    }
    values
}

/// The guest's square roots, from the runtime, against the native executor's.
#[test]
fn square_roots_agree_in_rv32_and_native() {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&SQRT_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);

            // 2^255 sits in the top byte of the 32 the host reads, so a
            // value handed over in the wrong order gets a different root.
            assert_eq!(
                client.sqrt(U256::ONE << 255),
                U256::from(0xb504_f333_f9de_6484_597d_89b3_754a_be9f_u128)
            );
            assert_eq!(
                client.sqrt(U256::ONE << 128),
                U256::from(1_u128 << 64)
            );
            assert_eq!(client.sqrt(U256::MAX), U256::from(u128::MAX));
            // The one root that does not fit 128 bits, which only the
            // rounding variant can return.
            assert_eq!(
                client.sqrt_ceil(U256::MAX),
                U256::from(u128::MAX) + U256::ONE
            );

            for value in sqrt_values() {
                let floor = value.integer_sqrt();
                assert_eq!(client.sqrt(value), floor, "{value:?}");
                assert_eq!(
                    client.sqrt_ceil(value),
                    value.sqrt_rounding(Rounding::Ceil),
                    "{value:?}"
                );
                // The definition, so the software side is not taken on
                // trust here either: floor^2 <= value < (floor + 1)^2.
                assert!(floor * floor <= value, "{value:?}");
                let above = floor + U256::ONE;
                assert!(
                    above.checked_mul(above).is_none_or(|sq| sq > value),
                    "{value:?}"
                );
            }
        })
    })
    .assert_success();
}

/// A xorshift generator, so the operands are the same on every run.
fn xorshift(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

/// A value of exactly `bits` bits, zero for `bits == 0`, with its other bits
/// scattered.
fn of_bit_length(bits: u32, next: &mut impl FnMut() -> u64) -> U256 {
    if bits == 0 {
        return U256::ZERO;
    }
    let noise = U256::from_limbs(core::array::from_fn(|_| next() as u32));
    let top = U256::ONE << (bits - 1);
    top | (noise & (top - U256::ONE))
}

/// The low 128 bits of `value`.
fn low_u128(value: U256) -> u128 {
    u128::try_from(value & U256::from(u128::MAX)).unwrap()
}

/// Operands a multiply-divide can get wrong: the ends of the range, a zero
/// divisor, quotients either side of 2^256, and products and divisors of
/// every length, where the length of the divisor picks the division's path.
fn mul_div_operands() -> Vec<(U256, U256, U256)> {
    let top = U256::ONE << 255;
    let mut operands = vec![
        (U256::ZERO, U256::MAX, U256::ONE),
        (U256::MAX, U256::MAX, U256::ZERO),
        (U256::MAX, U256::MAX, U256::MAX),
        (U256::MAX, U256::MAX, U256::MAX - U256::ONE),
        (U256::MAX, U256::ONE, U256::ONE),
        (top, U256::from(2), U256::ONE),
        (top, U256::from(2), U256::from(2)),
        (top, U256::from(2), U256::from(3)),
        (test_value(), test_value(), U256::from(u128::MAX)),
        (test_value(), U256::MAX, test_value()),
    ];
    let mut next = xorshift(0x2545_f491_4f6c_dd1d);
    // A divisor of every length, under products of full width (which mostly
    // overflow, leaving the remainder) and of a length that keeps the
    // quotient in range.
    for bits in 1..=256_u32 {
        let d = of_bit_length(bits, &mut next);
        let x = of_bit_length(256, &mut next);
        operands.push((x, of_bit_length(256, &mut next), d));
        operands.push((x, of_bit_length(bits - 1, &mut next), d));
        operands.push((of_bit_length(bits, &mut next), x, d));
    }
    operands
}

/// The guest's divisions, from the runtime, against the native executor's.
#[test]
fn divisions_agree_in_rv32_and_native() {
    let contract = load_rv32_contract(RV32_PATH);
    let env = TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&MUL_DIV_NONCE, &contract_hash);
            let client = U256RoundtripClient::new(&addr);
            let top = U256::ONE << 255;
            let u128_top = 1_u128 << 127;

            // Values whose bytes in the wrong order give another answer.
            assert_eq!(
                client.mul_div(top, U256::from(3), U256::from(2)),
                Some(U256::from(3) << 254)
            );
            assert_eq!(
                client.mul_div(U256::MAX, U256::MAX, U256::MAX),
                Some(U256::MAX)
            );
            // The quotient is 2^256, one past the range, with remainder 1.
            assert_eq!(
                client.mul_div(U256::MAX, U256::MAX, U256::MAX - U256::ONE),
                None
            );
            assert_eq!(client.mul_div(top, U256::from(2), U256::ONE), None);
            assert_eq!(client.mul_div(U256::ONE, U256::ONE, U256::ZERO), None);
            assert_eq!(
                client.mul_div(U256::ONE, U256::ONE, U256::from(3)),
                Some(U256::ZERO)
            );
            assert_eq!(
                client.mul_div_ceil(U256::ONE, U256::ONE, U256::from(3)),
                Some(U256::ONE)
            );
            // hogd's bridge test example: the quotient overflows, and the
            // remainder is 5.
            let a = top + (U256::ONE << 64) + U256::from(5);
            let b = (U256::ONE << 65) + U256::ONE;
            assert_eq!(client.mul_div(a, b, U256::ONE << 64), None);
            assert_eq!(
                client.mul_mod(a, b, U256::ONE << 64),
                Some(U256::from(5))
            );
            assert_eq!(
                client.mul_mod(U256::MAX, U256::MAX, U256::from(7)),
                Some(U256::ONE)
            );
            assert_eq!(client.mul_mod(U256::MAX, U256::MAX, U256::ZERO), None);
            assert_eq!(
                client.div(U256::MAX, U256::ONE << 128),
                U256::from(u128::MAX)
            );
            assert_eq!(
                client.rem(U256::MAX, U256::ONE << 128),
                U256::from(u128::MAX)
            );
            assert_eq!(
                client.div_small(U256::from(1_000_000_000_007), 1_000),
                U256::from(1_000_000_000)
            );
            assert_eq!(
                client.signed_div(I256::from(-7), I256::from(2)),
                I256::from(-3)
            );
            assert_eq!(
                client.signed_mul_div(I256::MIN, I256::from(-1), I256::ONE),
                None
            );
            assert_eq!(
                client.signed_mul_div(
                    I256::MIN,
                    I256::from(-1),
                    I256::from(-1)
                ),
                Some(I256::MIN)
            );
            assert_eq!(
                client.signed_mul_div(
                    I256::from(-7),
                    I256::from(3),
                    I256::from(2)
                ),
                Some(I256::from(-10))
            );
            assert_eq!(
                client.mul_div_u128(u128::MAX, u128::MAX, u128::MAX),
                Some(u128::MAX)
            );
            assert_eq!(client.mul_div_u128(u128_top, 4, 2), None);
            assert_eq!(
                client.mul_div_u128(u128_top, 2, 4),
                Some(1_u128 << 126)
            );
            assert_eq!(client.mul_div_u128(1, 1, 0), None);
            assert_eq!(client.mul_div_i128(-7, 3, 2), Some(-10));
            assert_eq!(client.mul_div_i128_floor(-7, 3, 2), Some(-11));
            assert_eq!(client.mul_div_i128_floor(7, 3, 2), Some(10));
            assert_eq!(
                client.mul_div_i128(i128::MAX, i128::MIN, i128::MAX),
                Some(i128::MIN)
            );
            assert_eq!(client.mul_div_i128(i128::MIN, -1, 1), None);
            assert_eq!(client.mul_div_i128(1, 1, 0), None);
            assert_eq!(client.mul_div_i128_floor(1, 1, 0), None);
            // 2^255 - 19 is prime; the exponent is a full 256 bits.
            let p = top - U256::from(19);
            assert_eq!(
                client.mod_exp(test_value(), U256::MAX, p),
                Some(
                    (U256::from(
                        0x6ee7_0e46_a989_5115_cdd4_0ad4_cfd7_696d_u128
                    ) << 128)
                        | U256::from(
                            0xaf0a_e4e2_9333_0367_0c73_d3f3_8af2_b682_u128
                        )
                )
            );
            assert_eq!(
                client.mod_exp(
                    U256::from(3),
                    (U256::ONE << 200) + U256::from(12345),
                    U256::MAX
                ),
                Some(
                    (U256::from(
                        0x0e41_36c8_457b_9956_14f2_605c_0052_c6cf_u128
                    ) << 128)
                        | U256::from(
                            0x8ca8_075e_93e4_c16a_1742_eaa1_5131_e4f7_u128
                        )
                )
            );
            assert_eq!(
                client.mod_exp(U256::from(3), U256::ONE, U256::ZERO),
                None
            );

            for (x, y, d) in mul_div_operands() {
                let quotient = U256::checked_mul_div(x, y, d);
                assert_eq!(
                    client.mul_div(x, y, d),
                    quotient,
                    "{x:?} {y:?} {d:?}"
                );
                assert_eq!(
                    client.mul_div_ceil(x, y, d),
                    U256::checked_mul_div_rounding(x, y, d, Rounding::Ceil),
                    "{x:?} {y:?} {d:?}"
                );
                let remainder = U256::checked_mul_mod(x, y, d);
                assert_eq!(
                    client.mul_mod(x, y, d),
                    remainder,
                    "{x:?} {y:?} {d:?}"
                );
                // The definition, so the software side is not taken on trust
                // here either: q * d + r == x * y, with r < d.
                if let (Some(q), Some(r)) = (quotient, remainder) {
                    assert!(r < d, "{x:?} {y:?} {d:?}");
                    let (lo, hi) = x.widening_mul(y);
                    let (qd_lo, qd_hi) = q.widening_mul(d);
                    let (sum, carry) = qd_lo.overflowing_add(r);
                    assert_eq!(
                        (sum, qd_hi + U256::from(u128::from(carry))),
                        (lo, hi)
                    );
                }
                if !d.is_zero() {
                    assert_eq!(client.div(x, d), x / d, "{x:?} {d:?}");
                    assert_eq!(client.rem(x, d), x % d, "{x:?} {d:?}");
                }
                // The same magnitudes with signs, from the bits of `x`.
                let (sx, sy, sd) = (
                    if x.bit(0) {
                        x.cast_signed().wrapping_neg()
                    } else {
                        x.cast_signed()
                    },
                    if x.bit(1) {
                        y.cast_signed().wrapping_neg()
                    } else {
                        y.cast_signed()
                    },
                    if x.bit(2) {
                        d.cast_signed().wrapping_neg()
                    } else {
                        d.cast_signed()
                    },
                );
                assert_eq!(
                    client.signed_mul_div(sx, sy, sd),
                    I256::checked_mul_div(sx, sy, sd),
                    "{sx:?} {sy:?} {sd:?}"
                );
                // Signed `/` wherever it is defined: not by zero, and not
                // `MIN / -1`.
                if !sd.is_zero() && !(sx == I256::MIN && sd == I256::from(-1))
                {
                    assert_eq!(
                        client.signed_div(sx, sd),
                        sx / sd,
                        "{sx:?} {sd:?}"
                    );
                }
                // `div_rem_small`, by the low 32 bits of `d`.
                let small = d.to_limbs()[0];
                if small != 0 {
                    assert_eq!(
                        client.div_small(x, small),
                        x.div_rem_small(small).0,
                        "{x:?} {small}"
                    );
                }
                // And the low halves, at 128 bits.
                let (a, b, c) = (low_u128(x), low_u128(y), low_u128(d));
                assert_eq!(
                    client.mul_div_u128(a, b, c),
                    checked_mul_div_u128(a, b, c),
                    "{a} {b} {c}"
                );
                // And those read as signed.
                let (a, b, c) = (a as i128, b as i128, c as i128);
                assert_eq!(
                    client.mul_div_i128(a, b, c),
                    checked_mul_div_i128(a, b, c),
                    "{a} {b} {c}"
                );
                assert_eq!(
                    client.mul_div_i128_floor(a, b, c),
                    checked_mul_div_rounding_i128(a, b, c, Rounding::Floor),
                    "{a} {b} {c}"
                );
            }
        })
    })
    .assert_success();
}
