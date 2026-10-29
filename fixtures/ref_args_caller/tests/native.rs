//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use ref_args_caller::RefArgsCallerClient;
use thruput_sdk::*;

static CALLEE_NONCE: SHA256Hash = sha256_static!("REF_ARGS_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash = sha256_static!("REF_ARGS_CALLER_NONCE");

#[test]
fn ref_args_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_hash = *ref_args::register_mock_contract(&env).as_hash();
    let caller_hash = *ref_args_caller::register_mock_contract(&env).as_hash();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
            RefArgsCallerClient::new(&caller_addr).run(&callee_addr);
        })
    })
    .assert_success();
}
