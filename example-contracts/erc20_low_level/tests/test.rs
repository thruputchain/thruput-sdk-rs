use erc20_low_level as _;

#[test]
fn run_test() {
    let env = sdk::TestEnvironment::no_cost();
    let test_contract =
        sdk::register_mock_package(&env, env!("CARGO_PKG_NAME"));
    let test_contract_hash = *test_contract.as_hash();

    erc20_native_test::test_erc20_contract(env, &test_contract_hash);
}
