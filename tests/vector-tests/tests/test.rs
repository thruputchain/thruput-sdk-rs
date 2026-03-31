use std::cell::Cell;
use std::panic::AssertUnwindSafe;

use sdk::*;
use vector_tests::*;

static NONCE: SHA256Hash = sdk::sha256_static!("VECTOR_TEST_NONCE");

/// Helper: set up the fibonacci contract in a test environment.
fn setup() -> (TestEnvironmentHandle, SHA256Hash) {
    let env = TestEnvironment::no_cost();
    let contract = env.register_mock(VectorTestContract::__mock);
    let contract_hash = env.setup_create_contract(&contract);
    (env, contract_hash)
}

#[test]
fn test_vector_basic_usage() {
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
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(1);
                client.push_value(2);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    result.assert_success();

    let result_expect_conflict = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(1);
                client.push_value(2);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    // vector push acquires exclusive lock on the vector size
    result_expect_conflict.assert_commit_failed_nondeterministically();

    env.advance_block();

    let read_result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                assert_eq!(client.get_value(0), 1);
                assert_eq!(client.get_value(1), 2);
            }))
    });
    read_result.assert_success();
}

#[test]
fn test_vector_pop() {
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

    // Push two values, pop one, verify
    let result = env.transact(|tx| {
        tx.call(&ContractAddress::named("caller"))
            .run(AssertUnwindSafe(|| {
                let contract_addr = addr.get();
                let client = VectorTestClient::new(&contract_addr);

                client.push_value(100);
                client.push_value(200);
                assert_eq!(client.values_len(), 2);

                client.pop_value();
                assert_eq!(client.values_len(), 1);
                assert_eq!(client.get_value(0), 100);
            }))
    });
    result.assert_success();
}
