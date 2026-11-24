use thruput_sdk::{CodeHash, DeployNonce, Named};

static CALLEE_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("BARE_IMPL_CALLEE_EXAMPLE_TEST_NONCE"),
);
static CALLER_NONCE: DeployNonce = DeployNonce::new(
    thruput_sdk::sha256_static!("BARE_IMPL_CALLER_EXAMPLE_TEST_NONCE"),
);

#[test]
fn cross_call_uses_self_type_as_default_interface_name() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = bare_impl_callee::register_mock_contract(&env);
    let callee_code =
        CodeHash::<bare_impl_callee::BareImplCalleeClient>::of_mock(
            &callee_contract,
        );

    let caller_contract = bare_impl_caller::register_mock_contract(&env);
    let caller_code =
        CodeHash::<bare_impl_caller::BareImplCallerClient>::of_mock(
            &caller_contract,
        );

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr = callee_code.deploy(&CALLEE_NONCE);
                let caller_addr = caller_code.deploy(&CALLER_NONCE);

                let client = caller_addr.client();
                assert_eq!(client.call_add_one(&callee_addr, 41), 42);

                let expected = thruput_contract_util::contract_entrypoint_id(
                    Some("BareImplCallee"),
                    bare_impl_callee::BareImplCalleeClient::INTERFACE_VERSION,
                    &thruput_contract_util::NormalizedMethodSignature::new(
                        "add_one",
                    ),
                );
                assert_eq!(
                    thruput_sdk::contract::ContractEntrypointID::from(
                        bare_impl_callee::BareImplCalleeEntrypoints::add_one,
                    ),
                    expected,
                );
            })
    })
    .assert_success();
}
