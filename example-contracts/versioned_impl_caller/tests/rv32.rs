//! `versioned_impl_caller` calling `versioned_impl`, both running as rv32
//! guests.
//!
//! Requires the lowered `versioned_impl` and `versioned_impl_caller`
//! artifacts: `scripts/dev.sh build-examples-rv32` writes them.

use thruput_sdk::*;
use versioned_impl_caller::VersionedImplCallerClient;

// Test working directory is the example contract directory.
const CALLEE_PATH: &str = "../../target/thruput/versioned_impl.rv32a";
const CALLER_PATH: &str = "../../target/thruput/versioned_impl_caller.rv32a";

static CALLEE_NONCE: SHA256Hash =
    sha256_static!("VERSIONED_IMPL_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash =
    sha256_static!("VERSIONED_IMPL_CALLER_NONCE");

#[test]
fn both_versions_work_guest_to_guest_rv32() {
    let callee = load_rv32_contract(CALLEE_PATH);
    let caller = load_rv32_contract(CALLER_PATH);

    let env = TestEnvironment::no_cost();
    let callee_hash = env.setup_create_contract(&callee);
    let caller_hash = env.setup_create_contract(&caller);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
            VersionedImplCallerClient::new(&caller_addr).run(&callee_addr);
        })
    })
    .assert_success();
}
