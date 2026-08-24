//! End-to-end `sdk::U256` coverage through a real rv32 artifact.
//!
//! Complements `crates/sdk/tests/u256.rs`, which drives U256 through a
//! host-side mock contract: here the compiled `.rv32a` image runs under the
//! rv32 interpreter, so the guest-side calldata decode, typed return
//! encode, and storage syscalls are the ones a deployed contract executes.
//!
//! Requires the lowered `u256_roundtrip` artifact: `scripts/dev.sh
//! build-examples-rv32` writes it.

use sdk::*;
use u256_roundtrip::U256RoundtripClient;

// Test working directory is the example contract directory.
const RV32_PATH: &str = "../../target/groundhog/u256_roundtrip.rv32a";

static NONCE: SHA256Hash = sdk::sha256_static!("U256_ROUNDTRIP_TEST_NONCE");

/// A value whose limbs all differ and whose top bit is set, so any limb
/// reordering or truncation in the calldata/return/storage paths shows up.
fn test_value() -> U256 {
    U256::widening_mul(
        0xDEAD_BEEF_0011_2233_4455_6677_8899_AABB_u128,
        0x8000_0000_0000_0000_0000_0000_0000_0001_u128,
    )
}

/// U256 into the guest as an entrypoint argument, back out through the
/// typed return, and persisted in `Value<U256>` across a block boundary.
#[test]
fn u256_roundtrips_through_rv32_entrypoint_and_storage() {
    let contract = sdk::load_rv32_contract(RV32_PATH);
    let env = sdk::TestEnvironment::no_cost();
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
