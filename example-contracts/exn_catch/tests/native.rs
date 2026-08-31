mod common;

use sdk::*;

#[test]
fn native_exn_catch() {
    let env = sdk::TestEnvironment::default();
    let test_contract = exn_catch::register_mock_contract(&env);
    let contract_hash = *test_contract.as_hash();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test"))
            .run(move || common::exn_catch_test(&contract_hash))
    })
    .assert_success();
}
