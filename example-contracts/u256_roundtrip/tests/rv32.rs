//! End-to-end `thruput_sdk::U256` and `thruput_sdk::I256` coverage through a
//! real rv32 artifact.
//!
//! Complements `crates/sdk/tests/u256.rs`, which drives U256 through a
//! host-side mock contract: here the compiled `.rv32a` image runs under the
//! rv32 interpreter, so the guest-side calldata decode, typed return
//! encode, and storage syscalls are the ones a deployed contract executes.
//!
//! Requires the lowered `u256_roundtrip` artifact: `scripts/dev.sh
//! build-examples-rv32` writes it.

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

/// A value whose limbs all differ and whose top bit is set, so any limb
/// reordering or truncation in the calldata/return/storage paths shows up.
fn test_value() -> U256 {
    U256::widening_mul(
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
