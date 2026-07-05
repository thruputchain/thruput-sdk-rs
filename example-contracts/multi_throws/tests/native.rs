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
fn native_multi_throws_relay_preserves_payload() {
    let env = sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_relay_preserves_payload(&contract_hash)
        })
    })
    .assert_success();
}

#[test]
fn native_multi_throws_relay_payload_readable_after_interleave() {
    let env = sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let contract_hash = test_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_relay_payload_readable_after_interleave(
                &contract_hash,
            )
        })
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
