use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use complex_storage::*;

static NONCE: SHA256Hash = sdk::sha256_static!("COMPLEX_STORAGE_NONCE");

/// Helper: set up the fibonacci contract in a test environment.
fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = TestEnvironment::no_cost();
    let contract = env.register_mock(ComplexStorage::__mock);
    let contract_hash = env.setup_create_contract(&contract);
    (env, contract_hash)
}

#[test]
fn test_basic_usage() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = ComplexStorageClient::new(&contract_addr);

                client.push_inner();
            }))
    });
    result.assert_success();
}

#[test]
fn test_persistence_across_blocks() {
    let (env, contract_hash) = setup();
    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });
    deploy_result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = ComplexStorageClient::new(&contract_addr);
                client.push_inner();
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = ComplexStorageClient::new(&contract_addr);
                let balance = client.get_inner_balance([0; 32].into());
                assert_eq!(balance, 100);
            }))
    });
    result.assert_success();
}

#[test]
fn test_load_mut() {
    let (env, contract_hash) = setup();

    let addr = Cell::new(ContractAddress::default());

    let deploy_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                addr.set(sdk::deploy_contract(&NONCE, &contract_hash));
            }))
    });

    deploy_result.assert_success();
    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = ComplexStorageClient::new(&contract_addr);
                client.set_unconditional(12);
                client.set_value(10);
            }))
    });
    result.assert_success();

    env.advance_block();

    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = ComplexStorageClient::new(&contract_addr);
                let value = client.get_value();
                assert_eq!(value, 10);
            }))
    });
    result.assert_success();
}
