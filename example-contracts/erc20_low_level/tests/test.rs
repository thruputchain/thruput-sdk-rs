#[test]
fn run_test() {
    let mut env = sdk::TestEnvironment::no_cost();
    let test_contract = erc20_low_level::test::register(&mut env);
    let test_contract_hash = env.setup_create_contract(&test_contract);

    erc20_native_test::test_erc20_contract(env, &test_contract_hash);
}
