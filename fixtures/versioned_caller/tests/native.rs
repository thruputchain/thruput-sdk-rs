use thruput_sdk::{CodeHash, DeployNonce, Named};

static CALLEE_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("VERSIONED_CALLEE_EXAMPLE_TEST_NONCE"),
);
static CALLER_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("VERSIONED_CALLER_EXAMPLE_TEST_NONCE"),
);

#[test]
fn cross_call_uses_explicit_trait_version_entrypoint() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = versioned_callee::register_mock_contract(&env);
    let callee_code =
        CodeHash::<versioned_callee::VersionedCalleeClient>::of_mock(
            &callee_contract,
        );

    let caller_contract = versioned_caller::register_mock_contract(&env);
    let caller_code =
        CodeHash::<versioned_caller::VersionedCallerClient>::of_mock(
            &caller_contract,
        );

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr = callee_code.deploy(&CALLEE_NONCE);
                let caller_addr = caller_code.deploy(&CALLER_NONCE);

                let client = caller_addr.client();
                assert_eq!(client.call_add_one(&callee_addr, 41), 42);
            })
    })
    .assert_success();
}
