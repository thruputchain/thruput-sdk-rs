mod common;

use sdk::*;

#[test]
fn native_multi_throws() {
    let env = sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::multi_throws_test(&contract_hash))
    })
    .assert_success();
}

#[test]
fn native_multi_throws_undecodable() {
    let env = sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::multi_throws_test_undecodable(&contract_hash))
    })
    .assert_success();
}

#[test]
fn native_multi_throws_malicious_library_error() {
    let env = sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_malicious_library_error(&contract_hash)
        })
    })
    .assert_success();
}
