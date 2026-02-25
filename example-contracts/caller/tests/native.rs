mod common;
use common::*;
use sdk::TestRegister;

// Load two contracts into the same test environment, and
// have one call the other.
#[test]
fn caller_test_native() {
    let mut env = sdk::TestEnvironment::no_cost();

    let erc20_contract = erc20::TokenContract::register(&mut env);
    let erc20_hash = env.setup_create_contract(&erc20_contract);

    let caller_contract = caller::Caller::register(&mut env);
    let caller_hash = env.setup_create_contract(&caller_contract);

    sdk::contract_exec(env, move || {
        caller_test_inner(&erc20_hash, &caller_hash);
    })
    .unwrap_or_else(|e| panic!("Caller test failed: {}", e));
}
