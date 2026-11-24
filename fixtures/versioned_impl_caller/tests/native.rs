//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use thruput_sdk::*;
use versioned_impl_caller::VersionedImplCallerClient;

static CALLEE_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("VERSIONED_IMPL_CALLEE_NONCE"));
static CALLER_NONCE: DeployNonce =
    DeployNonce::new(sha256_static!("VERSIONED_IMPL_CALLER_NONCE"));

#[test]
fn both_versions_work_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_code =
        CodeHash::<(adder_v1::AdderClient, adder_v2::AdderClient)>::of_mock(
            &versioned_impl::register_mock_contract(&env),
        );
    let caller_code = CodeHash::<VersionedImplCallerClient>::of_mock(
        &versioned_impl_caller::register_mock_contract(&env),
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
