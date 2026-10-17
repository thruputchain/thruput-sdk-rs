//! Test one contract calling the other, both running as rv32 guests.
//!
//! Requires the lowered `erc20` and `caller` artifacts:
//! `scripts/dev.sh build-examples-rv32` writes them.
mod common;
use common::*;
use thruput_sdk::Named;

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
