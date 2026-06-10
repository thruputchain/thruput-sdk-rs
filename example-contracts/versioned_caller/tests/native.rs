use sdk::Named;

static CALLEE_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("VERSIONED_CALLEE_EXAMPLE_TEST_NONCE");
static CALLER_NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("VERSIONED_CALLER_EXAMPLE_TEST_NONCE");

#[test]
fn cross_call_uses_explicit_trait_version_entrypoint() {
    let env = sdk::TestEnvironment::no_cost();

    let callee_contract = versioned_callee::register_mock_contract(&env);
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract = versioned_caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&sdk::ContractAddress::named("test")).run(move || {
            let callee_addr =
                sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
            let caller_addr =
                sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

            let client =
                versioned_caller::VersionedCallerClient::new(&caller_addr);
            assert_eq!(client.call_add_one(&callee_addr, 41), 42);
        })
    })
    .assert_success();
}
