use thruput_sdk::Named;

static CALLEE_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("IMPORTED_ERROR_CALLEE_TEST_NONCE");
static CALLER_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("IMPORTED_ERROR_CALLER_TEST_NONCE");

#[test]
fn cross_call_decodes_reexported_imported_error_without_direct_dependency() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = imported_error_callee::register_mock_contract(&env);
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract = imported_error_caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr =
                    thruput_sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
                let caller_addr =
                    thruput_sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

                let client =
                    imported_error_caller::ImportedErrorCallerClient::new(
                        &caller_addr,
                    );
                assert_eq!(client.classify_failure(&callee_addr), 1);
            })
    })
    .assert_success();
}
