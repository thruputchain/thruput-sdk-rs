#[test]
fn run_test() {
    let mut env = sdk::TestEnvironment::new(None);
    let test_contract = erc20::implementation::test_setup::register(&mut env);

    erc20_native_test::test_erc20_contract(&env, &test_contract);
}
