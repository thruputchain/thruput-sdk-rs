//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use ref_args_caller::RefArgsCallerClient;
use thruput_sdk::*;

static CALLEE_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("REF_ARGS_CALLEE_NONCE"));
static CALLER_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("REF_ARGS_CALLER_NONCE"));

#[test]
fn ref_args_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_code = CodeHash::<ref_args::RefArgsClient>::of_mock(
        &ref_args::register_mock_contract(&env),
    );
    let caller_code = CodeHash::<RefArgsCallerClient>::of_mock(
        &ref_args_caller::register_mock_contract(&env),
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
