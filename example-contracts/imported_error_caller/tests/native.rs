static CALLEE_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("IMPORTED_ERROR_CALLEE_TEST_NONCE");
static CALLER_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("IMPORTED_ERROR_CALLER_TEST_NONCE");

#[test]
fn cross_call_decodes_reexported_imported_error_without_direct_dependency() {
    let env = sdk::TestEnvironment::no_cost();

    let callee_contract = env.register_mock(
        <imported_error_callee::ImportedErrorCalleeContract as imported_error_callee::ImportedErrorCallee>::__mock,
    );
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract =
        env.register_mock(imported_error_caller::ImportedErrorCaller::__mock);
    let caller_hash = caller_contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        let callee_addr = sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
        let caller_addr = sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

        let client = imported_error_caller::ImportedErrorCallerClient::new(
            &caller_addr,
        );
        assert_eq!(client.classify_failure(&callee_addr), 1);
    })
    .unwrap_or_else(|e| panic!("Imported error caller test failed: {}", e));
}
