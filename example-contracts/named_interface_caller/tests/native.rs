static CALLEE_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("NAMED_INTERFACE_CALLEE_EXAMPLE_TEST_NONCE");
static CALLER_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("NAMED_INTERFACE_CALLER_EXAMPLE_TEST_NONCE");

#[test]
fn cross_call_uses_explicit_trait_interface_entrypoint() {
    let env = sdk::TestEnvironment::no_cost();

    let callee_contract =
        named_interface_callee::__groundhog_register_mock_package(&env);
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract =
        named_interface_caller::__groundhog_register_mock_package(&env);
    let caller_hash = caller_contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        let callee_addr = sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
        let caller_addr = sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

        let client = named_interface_caller::NamedInterfaceCallerClient::new(
            &caller_addr,
        );
        assert_eq!(client.call_add_one(&callee_addr, 41), 42);
    })
    .unwrap_or_else(|e| panic!("Named interface caller test failed: {}", e));
}
