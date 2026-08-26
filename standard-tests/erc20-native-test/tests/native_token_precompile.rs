//! Regression test: the native token precompile (pre-deployed at
//! `ADDRESS_NATIVE_TOKEN` by genesis) stays compatible with the
//! standard ERC20 interface where it can, and keeps its documented
//! divergences where it can't.

#[test]
fn run_test() {
    let env = sdk::TestEnvironment::no_cost();
    erc20_native_test::test_native_token_precompile(env);
}
