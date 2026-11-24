use thruput_sdk::{CodeHash, DeployNonce, Named};

static CALLEE_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("IMPORTED_ERROR_CALLEE_TEST_NONCE"),
);
static CALLER_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("IMPORTED_ERROR_CALLER_TEST_NONCE"),
);

#[test]
fn cross_call_decodes_reexported_imported_error_without_direct_dependency() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = imported_error_callee::register_mock_contract(&env);
    let callee_code = CodeHash::<
        imported_error_callee::ImportedErrorCalleeClient,
    >::of_mock(&callee_contract);

    let caller_contract = imported_error_caller::register_mock_contract(&env);
    let caller_code = CodeHash::<
        imported_error_caller::ImportedErrorCallerClient,
    >::of_mock(&caller_contract);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr = callee_code.deploy(&CALLEE_NONCE);
                let caller_addr = caller_code.deploy(&CALLER_NONCE);

                let client = caller_addr.client();
                assert_eq!(client.classify_failure(&callee_addr), 1);
            })
    })
    .assert_success();
}
