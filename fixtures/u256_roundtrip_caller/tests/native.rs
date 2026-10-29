//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use thruput_sdk::*;
use u256_roundtrip_caller::U256RoundtripCallerClient;

static CALLEE_NONCE: SHA256Hash =
    sha256_static!("U256_ROUNDTRIP_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash =
    sha256_static!("U256_ROUNDTRIP_CALLER_NONCE");

#[test]
fn u256_roundtrips_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_hash = *u256_roundtrip::register_mock_contract(&env).as_hash();
    let caller_hash =
        *u256_roundtrip_caller::register_mock_contract(&env).as_hash();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
            U256RoundtripCallerClient::new(&caller_addr).run(&callee_addr);
        })
    })
    .assert_success();
}
