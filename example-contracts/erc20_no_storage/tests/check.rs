#[test]
fn run_test() {
    let env = sdk::TestEnvironment::no_cost();
    let test_contract =
        erc20_no_storage::__groundhog_register_mock_package(&env);

    erc20_native_test::test_erc20_contract(env, test_contract.as_hash());
}
