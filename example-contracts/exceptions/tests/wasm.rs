#![allow(unused)] // XXX

// test working directory is example contract directory
const WASM_PATH: &str = "../../target/optimized/exceptions.wasm";

mod common;
use common::*;
use core::convert::Infallible;

#[test]
fn wasm_ok() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::default();
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
fn wasm_deploy() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        let _addr = sdk::deploy_contract(&TEST_NONCE, &contract_hash);
    });

    if let Err(err) = result {
        panic!("Exceptions test failed: {}", err);
    }
}

#[test]
fn wasm_exceptions() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
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
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            panic_test(&contract_hash)
        })
    });

    if let Err(err) = result {
        panic!("Panic test failed: {:?}", err);
    }
}

/*
#[test]
fn wasm_panic2() {
    let contract = sdk::load_wasm_contract(WASM_PATH);
    let env = sdk::TestEnvironment::no_cost();
    let contract_hash = env.setup_create_contract(&contract);

    let result = sdk::contract_test_invoke(env, move || {
        sdk::with_abort_handler(&sdk::test_abort_handler::<Infallible>, || {
            //
            let addr = deploy_contract(&TEST_NONCE, contract_hash);
            //
            let mut client = ContractClient::new(&addr);

            match client.try_panics() {
                Err(CallError::Fault(f)) => {
                    println!("OK: Got fault: {}", f);
                }
                Err(e) => {
                    panic!("Unexpected error: {}", e);
                }
                Ok(()) => {
                    panic!("Unexpected success")
                }
            }
        })
    });

    if let Err(err) = result {
        panic!("Panic test failed: {:?}", err);
    }
}
*/
