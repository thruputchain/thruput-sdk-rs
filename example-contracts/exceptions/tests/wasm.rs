// test working directory is example contract directory
const WASM_PATH: &str = "../../target/optimized/exceptions.wasm";

mod common;
use common::*;
use sdk::*;

#[test]
fn wasm_ok() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = TestEnvironment::default();
    let _ = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(|| {
            // Do nothing
        })
    })
    .assert_success();
}

#[test]
fn wasm_deploy() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let _addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);
        })
    })
    .assert_success();
}

#[test]
fn wasm_exceptions() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || exceptions_test(&contract_hash))
    })
    .assert_success();
}

#[test]
fn wasm_panic() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || panic_test(&contract_hash))
    })
    .assert_success();
}
