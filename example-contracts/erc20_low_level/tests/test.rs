#[test]
fn run_test() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let test_contract = erc20_low_level::register_mock_contract(&env);
    let test_contract_hash = *test_contract.as_hash();

    erc20_native_test::test_erc20_contract(env, &test_contract_hash);
}
