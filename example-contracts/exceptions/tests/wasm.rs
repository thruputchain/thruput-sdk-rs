#![allow(unused)] // XXX

// test working directory is example contract directory
const WASM_PATH: &str = "../../target/optimized/exceptions.wasm";

mod common;
use common::*;
use core::convert::Infallible;
use sdk::*;

#[test]
fn wasm_ok() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = TestEnvironment::default();
    let _ = env.setup_create_contract(&contract);

    contract_exec(env, || {
        // Do nothing
    })
    .unwrap_or_else(|e| panic!("Ok test failed: {e:?}"));
}

#[test]
fn wasm_deploy() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    contract_exec(env, move || {
        let _addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);
    })
    .unwrap_or_else(|e| panic!("Deploy test failed: {e:?}"));
}

#[test]
fn wasm_exceptions() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    contract_exec(env, move || exceptions_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Exceptions test failed: {e:?}"));
}

#[test]
fn wasm_panic() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    contract_exec(env, move || panic_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Panic test failed: {e:?}"));
}
