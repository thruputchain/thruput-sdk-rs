//! `u256_roundtrip_caller` calling `u256_roundtrip`, both running as rv32
//! guests.
//!
//! Requires the lowered `u256_roundtrip` and `u256_roundtrip_caller`
//! artifacts: `cargo gnaw build u256_roundtrip u256_roundtrip_caller` writes
//! them.

use thruput_sdk::*;
use u256_roundtrip_caller::U256RoundtripCallerClient;

// Test working directory is the example contract directory.
const CALLEE_PATH: &str = "../../target/thruput/u256_roundtrip.rv32a";
const CALLER_PATH: &str = "../../target/thruput/u256_roundtrip_caller.rv32a";

static CALLEE_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("U256_ROUNDTRIP_CALLEE_NONCE"));
static CALLER_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("U256_ROUNDTRIP_CALLER_NONCE"));

#[test]
fn u256_roundtrips_guest_to_guest_rv32() {
    let callee = load_rv32_contract(CALLEE_PATH);
    let caller = load_rv32_contract(CALLER_PATH);

    let env = TestEnvironment::no_cost();
    let callee_code = CodeHash::<u256_roundtrip::U256RoundtripClient>::new(
        env.setup_create_contract(&callee),
    );
    let caller_code = CodeHash::<U256RoundtripCallerClient>::new(
        env.setup_create_contract(&caller),
    );

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = callee_code.deploy(&CALLEE_NONCE);
            let caller_addr = caller_code.deploy(&CALLER_NONCE);
            caller_addr.client().run(&callee_addr);
        })
    })
    .assert_success();
}
