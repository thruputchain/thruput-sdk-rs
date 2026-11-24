//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use thruput_sdk::*;
use u256_roundtrip_caller::U256RoundtripCallerClient;

static CALLEE_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("U256_ROUNDTRIP_CALLEE_NONCE"));
static CALLER_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("U256_ROUNDTRIP_CALLER_NONCE"));

#[test]
fn u256_roundtrips_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_code = CodeHash::<u256_roundtrip::U256RoundtripClient>::of_mock(
        &u256_roundtrip::register_mock_contract(&env),
    );
    let caller_code = CodeHash::<U256RoundtripCallerClient>::of_mock(
        &u256_roundtrip_caller::register_mock_contract(&env),
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
