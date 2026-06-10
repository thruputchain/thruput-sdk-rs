mod common;
use common::*;
use sdk::Named;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let env = sdk::TestEnvironment::no_cost();

    let erc20_contract = erc20::register_mock_contract(&env);
    let erc20_hash = erc20_contract.as_hash().clone();

    let caller_contract = caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&sdk::ContractAddress::named("test")).run(move || {
            caller_test_inner(&erc20_hash, &caller_hash);
        })
    })
    .assert_success();
}
