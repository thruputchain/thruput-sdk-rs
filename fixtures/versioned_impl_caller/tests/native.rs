//! The checks `tests/rv32.rs` runs, with both contracts as host mocks: when
//! the rv32 test faults and this one passes, the guest path is what broke.

use thruput_sdk::*;
use versioned_impl_caller::VersionedImplCallerClient;

static CALLEE_NONCE: SHA256Hash =
    sha256_static!("VERSIONED_IMPL_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash =
    sha256_static!("VERSIONED_IMPL_CALLER_NONCE");

#[test]
fn both_versions_work_caller_to_callee_native() {
    let env = TestEnvironment::no_cost();
    let callee_hash = *versioned_impl::register_mock_contract(&env).as_hash();
    let caller_hash =
        *versioned_impl_caller::register_mock_contract(&env).as_hash();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
            VersionedImplCallerClient::new(&caller_addr).run(&callee_addr);
        })
    })
    .assert_success();
}
