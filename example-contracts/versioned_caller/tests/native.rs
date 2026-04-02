static CALLEE_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("VERSIONED_CALLEE_EXAMPLE_TEST_NONCE");
static CALLER_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("VERSIONED_CALLER_EXAMPLE_TEST_NONCE");

#[test]
fn cross_call_uses_explicit_trait_version_entrypoint() {
    let env = sdk::TestEnvironment::no_cost();

    let callee_contract = env.register_mock(
        <versioned_callee::VersionedCalleeContract as versioned_callee::VersionedCallee>::__mock,
    );
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract =
        env.register_mock(versioned_caller::VersionedCaller::__mock);
    let caller_hash = caller_contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        let callee_addr = sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
        let caller_addr = sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

        let client =
            versioned_caller::VersionedCallerClient::new(&caller_addr);
        assert_eq!(client.call_add_one(&callee_addr, 41), 42);
    })
    .unwrap_or_else(|e| panic!("Versioned caller test failed: {}", e));
}
