use thruput_sdk::Named;

static CALLEE_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("BARE_IMPL_CALLEE_EXAMPLE_TEST_NONCE");
static CALLER_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("BARE_IMPL_CALLER_EXAMPLE_TEST_NONCE");

#[test]
fn cross_call_uses_self_type_as_default_interface_name() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = bare_impl_callee::register_mock_contract(&env);
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract = bare_impl_caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr =
                    thruput_sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
                let caller_addr =
                    thruput_sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

                let client =
                    bare_impl_caller::BareImplCallerClient::new(&caller_addr);
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
