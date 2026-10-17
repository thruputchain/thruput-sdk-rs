//! Test one contract calling the other, both running as rv32 guests.
//!
//! Requires the lowered `erc20` and `caller` artifacts:
//! `scripts/dev.sh build-examples-rv32` writes them.
mod common;
use caller::CallerClient;
use common::*;
use std::sync::{Arc, Mutex};
use thruput_erc20_if::ERC20Client;
use thruput_sdk::{
    ContractAddress, FrameAction, FrameHandle, Named, SHA256Hash,
    deploy_contract, get_current_address,
};

// These paths only work for the example-contracts directory
// that is hosted in the SDK package.
//
const ERC20_PATH: &str = "../../target/thruput/erc20.rv32a";
const CALLER_PATH: &str = "../../target/thruput/caller.rv32a";

#[test]
fn caller_test_rv32() {
    let erc20_contract = thruput_sdk::load_rv32_contract(ERC20_PATH);
    let caller_contract = thruput_sdk::load_rv32_contract(CALLER_PATH);

    let env = thruput_sdk::TestEnvironment::no_cost();

    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);

    let result = env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                caller_test_inner(&erc20_hash, &caller_hash);
            })
    });
    result.assert_success();

    // The diagnostic reports the body's most recent rv32 call: the caller
    // contract's `check_balance`, which established a stack and used some.
    let usage = result
        .rv32_stack_usage()
        .expect("the test body invoked rv32 contracts");
    assert!(usage.wrote_sp(), "the guest's _start sets sp: {usage:?}");
    let bytes_used = usage.bytes_used().unwrap();
    assert!(bytes_used > 0, "a call uses some stack: {usage:?}");
    println!(
        "caller.check_balance stack usage: {bytes_used} bytes ({usage:?})"
    );
}

static STACK_ERC20_NONCE: SHA256Hash =
    thruput_sdk::sha256_static!("ERC20_STACK_USAGE_NONCE");
static STACK_CALLER_NONCE: SHA256Hash =
    thruput_sdk::sha256_static!("CALLER_STACK_USAGE_NONCE");

/// The stack records are each frame's own. A frame that runs no rv32 code
/// reports none even after other frames of the transaction have; a frame
/// that does reports its calls and nothing else's; and a parent's own calls
/// are not displaced by its children's.
#[test]
fn rv32_stack_usage_is_per_frame() {
    let erc20_contract = thruput_sdk::load_rv32_contract(ERC20_PATH);
    let caller_contract = thruput_sdk::load_rv32_contract(CALLER_PATH);

    let env = thruput_sdk::TestEnvironment::no_cost();
    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);

    // The addresses the body deploys to, for the assertions after it. An
    // `Arc<Mutex<_>>` rather than a `Cell`: the body must be `UnwindSafe`.
    let deployed: Arc<Mutex<Option<(ContractAddress, ContractAddress)>>> =
        Arc::new(Mutex::new(None));
    let deployed_slot = Arc::clone(&deployed);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run_continuable(move |frame: &mut FrameHandle| {
                // The root body's own rv32 calls: one invocation each,
                // neither calling another contract.
                let erc20_addr =
                    deploy_contract(&STACK_ERC20_NONCE, &erc20_hash);
                ERC20Client::new(&erc20_addr)
                    .initialize(&get_current_address());
                let caller_addr =
                    deploy_contract(&STACK_CALLER_NONCE, &caller_hash);
                CallerClient::new(&caller_addr).initialize(&erc20_addr);
                *deployed_slot.lock().unwrap() =
                    Some((erc20_addr, caller_addr));

                // A child that runs no rv32 code has no record, although
                // the transaction holds two by now.
                let idle = frame
                    .child_call(&ContractAddress::named("idle"))
                    .run(|| ());
                idle.assert_success();
                assert_eq!(idle.rv32_stack_usage(), None);
                assert!(idle.rv32_stack_usage_records().is_empty());

                // A child that calls rv32 reports its own call, with the
                // erc20 call `check_balance` makes before it, and nothing
                // from the parent's body.
                let busy = frame
                    .child_call(&ContractAddress::named("busy"))
                    .run(move || {
                        let caller = CallerClient::new(&caller_addr);
                        assert_eq!(
                            0,
                            caller.check_balance(&ContractAddress::zero())
                        );
                    });
                busy.assert_success();
                let addresses: Vec<ContractAddress> = busy
                    .rv32_stack_usage_records()
                    .iter()
                    .map(|record| record.contract_address)
                    .collect();
                assert_eq!(addresses, [erc20_addr, caller_addr]);
                assert_eq!(
                    busy.rv32_stack_usage().unwrap().contract_address,
                    caller_addr
                );

                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();

    // The root frame's own records are its two `initialize` calls, in
    // order; the children's calls are not among them, so its most recent
    // is `caller.initialize`, not the child's `check_balance`.
    let (erc20_addr, caller_addr) = deployed.lock().unwrap().unwrap();
    let addresses: Vec<ContractAddress> = result
        .rv32_stack_usage_records()
        .iter()
        .map(|record| record.contract_address)
        .collect();
    assert_eq!(addresses, [erc20_addr, caller_addr]);
    assert_eq!(
        result.rv32_stack_usage().unwrap().contract_address,
        caller_addr
    );
}
