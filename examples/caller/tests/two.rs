//! Test one contract calling the other, both running in wasm
mod common;
use common::*;
use sdk::contract_test::load_wasm_contract;
use sdk::transaction::InvocationFrameStatusCode;

// test working directory is example contract directory
const ERC20_PATH: &str = "../../target/optimized/erc20.wasm";
const CALLER_PATH: &str = "../../target/optimized/caller.wasm";

#[test]
fn caller_test() {
    let erc20_contract = load_wasm_contract(ERC20_PATH);
    let caller_contract = load_wasm_contract(CALLER_PATH);

    let env = sdk::TestEnvironment::new(None);

    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let contract_hash = env.setup_create_contract(&caller_contract);

    let mut handler = env.prepare_test(move || -> () {
        caller_test_inner(&erc20_hash, &contract_hash);
    });

    let res = handler.root_invoke();
    assert_eq!(
        res.output.status_code(),
        InvocationFrameStatusCode::kInvocationFrameStatusCodeReturn as i32
    );
}
