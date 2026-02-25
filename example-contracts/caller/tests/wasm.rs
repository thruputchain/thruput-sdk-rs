//! Test one contract calling the other, both running in wasm
mod common;
use common::*;

// test working directory is example contract directory
const ERC20_PATH: &str = "../../target/optimized/erc20.wasm";
const CALLER_PATH: &str = "../../target/optimized/caller.wasm";

#[test]
fn caller_test_wasm() {
    let erc20_contract = sdk::load_wasm_contract(ERC20_PATH);
    let caller_contract = sdk::load_wasm_contract(CALLER_PATH);

    let env = sdk::TestEnvironment::no_cost();

    let erc20_hash = env.setup_create_contract(&erc20_contract);
    let caller_hash = env.setup_create_contract(&caller_contract);

    sdk::contract_exec(env, move || -> () {
        caller_test_inner(&erc20_hash, &caller_hash);
    })
    .unwrap_or_else(|e| panic!("Caller test failed: {}", e));
}
