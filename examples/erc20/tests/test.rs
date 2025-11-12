#[test]
fn check() {
    let mut env = sdk::TestEnvironment::new();
    let test_contract = erc20::implementation::test_setup::register(&mut env);

    erc20_result_test::test_erc20_contract(&env, &test_contract);
}
