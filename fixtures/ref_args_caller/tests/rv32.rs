//! `ref_args_caller` calling `ref_args`, both running as rv32 guests.
//!
//! Requires the lowered `ref_args` and `ref_args_caller` artifacts:
//! `cargo gnaw build ref_args ref_args_caller` writes them.

use ref_args_caller::RefArgsCallerClient;
use thruput_sdk::*;

// Test working directory is the example contract directory.
const CALLEE_PATH: &str = "../../target/thruput/ref_args.rv32a";
const CALLER_PATH: &str = "../../target/thruput/ref_args_caller.rv32a";

static CALLEE_NONCE: SHA256Hash = sha256_static!("REF_ARGS_CALLEE_NONCE");
static CALLER_NONCE: SHA256Hash = sha256_static!("REF_ARGS_CALLER_NONCE");

#[test]
fn ref_args_guest_to_guest_rv32() {
    let callee = load_rv32_contract(CALLEE_PATH);
    let caller = load_rv32_contract(CALLER_PATH);

    let env = TestEnvironment::no_cost();
    let callee_hash = env.setup_create_contract(&callee);
    let caller_hash = env.setup_create_contract(&caller);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let callee_addr = deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr = deploy_contract(&CALLER_NONCE, &caller_hash);
            RefArgsCallerClient::new(&caller_addr).run(&callee_addr);
        })
    })
    .assert_success();
}
