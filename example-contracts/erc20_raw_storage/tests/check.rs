#[test]
fn run_test() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let test_contract = erc20_raw_storage::register_mock_contract(&env);

    erc20_native_test::test_erc20_contract(env, test_contract.as_hash());
}
