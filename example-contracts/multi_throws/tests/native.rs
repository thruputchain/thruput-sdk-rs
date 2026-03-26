mod common;

use multi_throws::{Contract, MultiThrows};
use sdk::*;

#[test]
fn native_multi_throws() {
    let env = sdk::TestEnvironment::default();
    let test_contract = env.register_mock(<Contract as MultiThrows>::__mock);
    let contract_hash = env.setup_create_contract(&test_contract);

    contract_exec(env, move || common::multi_throws_test(&contract_hash))
        .unwrap_or_else(|e| panic!("Test failed: {:?}", e));
}
