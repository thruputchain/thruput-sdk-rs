#![allow(unused)] // XXX

// test working directory is example contract directory
const WASM_PATH: &str = "../../target/optimized/exceptions.wasm";

mod common;
use common::*;
use core::convert::Infallible;

#[test]
fn wasm_ok() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::new(None);
    let _ = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            // Do nothing
        })
    });

    if let Err(err) = result {
        panic!("Exceptions test failed: {}", err);
    }
}

#[test]
fn wasm_degenerate() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::new(None);
    let contract_hash = env.setup_create_contract(&contract);
    /*
    let contract_hash = &contract_hash;
    env.setup_grant_storage_tokens(deployed_addr, 1_000_000_000);
    */

    let result = sdk::contract_test_invoke(env, move || {
        /*
        let me = sdk::get_current_address();
        let deployed_addr = sdk::deploy_address(&me, &TEST_NONCE, contract_hash);
        let _addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);
        */
    });

    if let Err(err) = result {
        panic!("Exceptions test failed: {}", err);
    }
}

#[test]
fn wasm_exceptions() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::new(None);
    let contract_hash = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            exceptions_test(&contract_hash)
        })
    });

    if let Err(err) = result {
        panic!("Exceptions test failed: {}", err);
    }
}

#[test]
fn wasm_panic() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::new(None);
    let contract_hash = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            panic_test(&contract_hash)
        })
    });

    if let Err(err) = result {
        panic!("Panic test failed: {}", err);
    }
}
