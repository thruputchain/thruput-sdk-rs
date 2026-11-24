mod common;

use thruput_sdk::*;

#[test]
fn native_multi_throws() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::multi_throws_test(&code))
    })
    .assert_success();
}

#[test]
fn native_multi_throws_undecodable() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::multi_throws_test_undecodable(&code))
    })
    .assert_success();
}

#[test]
fn native_multi_throws_relay_preserves_payload() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_relay_preserves_payload(&code)
        })
    })
    .assert_success();
}

#[test]
fn native_multi_throws_relay_payload_readable_after_interleave() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_relay_payload_readable_after_interleave(
                &code,
            )
        })
    })
    .assert_success();
}

#[test]
fn native_multi_throws_relay_undecodable_twice() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_relay_undecodable_twice(&code)
        })
    })
    .assert_success();
}

#[test]
fn native_multi_throws_malicious_library_error() {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = multi_throws::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            common::multi_throws_test_malicious_library_error(&code)
        })
    })
    .assert_success();
}
