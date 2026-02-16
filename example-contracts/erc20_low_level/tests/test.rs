#[test]
fn run_test() {
    let env = sdk::TestEnvironment::new(Some(&sdk::StorageCostModel::no_cost_model()));
    let test_contract = erc20_low_level::test::register(&mut env);

    erc20_native_test::test_erc20_contract(&env, &test_contract);
}
