//! Test one contract calling the other, both running as rv32 guests.
//!
//! Requires the lowered `erc20` and `caller` artifacts:
//! `cargo gnaw build erc20 caller` writes them.
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

static NATIVE_ERC20_NONCE: SHA256Hash =
    thruput_sdk::sha256_static!("ERC20_NATIVE_FRAME_NONCE");
static NATIVE_CALLER_NONCE: SHA256Hash =
    thruput_sdk::sha256_static!("CALLER_NATIVE_FRAME_NONCE");
static NATIVE_MOCK_CALLER_NONCE: SHA256Hash =
    thruput_sdk::sha256_static!("MOCK_CALLER_NATIVE_FRAME_NONCE");

/// Invoking a native contract (a mock registered in this process, which
/// hogd's native runtime runs) records nothing, so a frame that invokes
/// one after an rv32 call elsewhere in the transaction, or in a later
/// transaction, reports `None`: it never inherits the rv32 call's record.
/// A frame whose own body made an rv32 call keeps reporting it after a
/// native call, and a native contract that itself calls an rv32 contract
/// reports that call.
#[test]
fn native_contract_frames_report_no_rv32_record() {
    let erc20_contract = thruput_sdk::load_rv32_contract(ERC20_PATH);
    let caller_contract = thruput_sdk::load_rv32_contract(CALLER_PATH);

    let env = thruput_sdk::TestEnvironment::no_cost();
    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);
    // The caller contract again, running natively as a mock.
    let mock_caller_hash = *caller::register_mock_contract(&env).as_hash();

    // (erc20, rv32 caller, native caller) addresses, for the later
    // transaction and the assertions after the body.
    let deployed: Arc<
        Mutex<Option<(ContractAddress, ContractAddress, ContractAddress)>>,
    > = Arc::new(Mutex::new(None));
    let deployed_slot = Arc::clone(&deployed);

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run_continuable(move |frame: &mut FrameHandle| {
                // The body's rv32 calls: initialize both, then check_balance.
                let erc20_addr =
                    deploy_contract(&NATIVE_ERC20_NONCE, &erc20_hash);
                ERC20Client::new(&erc20_addr)
                    .initialize(&get_current_address());
                let caller_addr =
                    deploy_contract(&NATIVE_CALLER_NONCE, &caller_hash);
                let caller = CallerClient::new(&caller_addr);
                caller.initialize(&erc20_addr);
                assert_eq!(0, caller.check_balance(&ContractAddress::zero()));

                // Then a native call from the same body: `initialize` on the
                // mock caller runs natively and calls no contract.
                let mock_caller_addr = deploy_contract(
                    &NATIVE_MOCK_CALLER_NONCE,
                    &mock_caller_hash,
                );
                let mock_caller = CallerClient::new(&mock_caller_addr);
                mock_caller.initialize(&erc20_addr);
                *deployed_slot.lock().unwrap() =
                    Some((erc20_addr, caller_addr, mock_caller_addr));

                // A child frame that only invokes the native contract: no
                // record, although the transaction holds four by now.
                let native = frame
                    .child_call(&ContractAddress::named("native"))
                    .run(move || {
                        CallerClient::new(&mock_caller_addr)
                            .initialize(&erc20_addr);
                    });
                native.assert_success();
                assert_eq!(native.rv32_stack_usage(), None);
                assert!(native.rv32_stack_usage_records().is_empty());

                // A child frame whose native call reaches an rv32 contract
                // (the mock caller's check_balance calls the rv32 erc20)
                // reports that rv32 call, and only it.
                let through_native = frame
                    .child_call(&ContractAddress::named("through_native"))
                    .run(move || {
                        assert_eq!(
                            0,
                            CallerClient::new(&mock_caller_addr)
                                .check_balance(&ContractAddress::zero())
                        );
                    });
                through_native.assert_success();
                let addresses: Vec<ContractAddress> = through_native
                    .rv32_stack_usage_records()
                    .iter()
                    .map(|record| record.contract_address)
                    .collect();
                assert_eq!(addresses, [erc20_addr]);

                FrameAction::Close
            })
            .finish()
    });
    result.assert_success();

    // The root's own records are its four rv32 calls, in finish order; the
    // native call after them added none, and the children's are excluded.
    // Its most recent is still the rv32 caller's check_balance.
    let (erc20_addr, caller_addr, mock_caller_addr) =
        deployed.lock().unwrap().unwrap();
    let addresses: Vec<ContractAddress> = result
        .rv32_stack_usage_records()
        .iter()
        .map(|record| record.contract_address)
        .collect();
    assert_eq!(
        addresses,
        [erc20_addr, caller_addr, erc20_addr, caller_addr]
    );
    assert_eq!(
        result.rv32_stack_usage().unwrap().contract_address,
        caller_addr
    );

    // A later transaction that only invokes the native contract: the
    // earlier transaction's records are not its. (A deployment becomes
    // visible to later transactions once the block advances.)
    env.advance_block();
    let later = env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            CallerClient::new(&mock_caller_addr).initialize(&erc20_addr);
        })
    });
    later.assert_success();
    assert_eq!(later.rv32_stack_usage(), None);
    assert!(later.rv32_stack_usage_records().is_empty());
}
