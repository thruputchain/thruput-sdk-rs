mod common;

use thruput_sdk::*;

#[test]
fn native_exn_catch() {
    run(common::exn_catch_test);
}

#[test]
fn native_exn_catch_mapped() {
    run(common::exn_catch_mapped_test);
}

fn run(test: fn(&SHA256Hash)) {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = exn_catch::register_mock_contract(&env);
    let contract_hash = *test_contract.as_hash();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || test(&contract_hash))
    })
    .assert_success();
}
