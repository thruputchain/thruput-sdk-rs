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

fn run(test: fn(&CodeHash<exn_catch::ExnCatchClient>)) {
    let env = thruput_sdk::TestEnvironment::default();
    let test_contract = exn_catch::register_mock_contract(&env);
    let code = CodeHash::of_mock(&test_contract);

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || test(&code))
    })
    .assert_success();
}
