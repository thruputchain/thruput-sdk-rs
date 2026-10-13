use thruput_sdk::Named;

static CALLEE_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("NAMED_INTERFACE_CALLEE_EXAMPLE_TEST_NONCE");
static CALLER_NONCE: thruput_sdk::SHA256Hash =
    thruput_sdk::sha256_static!("NAMED_INTERFACE_CALLER_EXAMPLE_TEST_NONCE");

#[test]
fn cross_call_uses_explicit_trait_interface_entrypoint() {
    let env = thruput_sdk::TestEnvironment::no_cost();

    let callee_contract = named_interface_callee::register_mock_contract(&env);
    let callee_hash = callee_contract.as_hash().clone();

    let caller_contract = named_interface_caller::register_mock_contract(&env);
    let caller_hash = caller_contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let callee_addr =
                    thruput_sdk::deploy_contract(&CALLEE_NONCE, &callee_hash);
                let caller_addr =
                    thruput_sdk::deploy_contract(&CALLER_NONCE, &caller_hash);

                let client =
                    named_interface_caller::NamedInterfaceCallerClient::new(
                        &caller_addr,
                    );
                assert_eq!(client.call_add_one(&callee_addr, 41), 42);
            })
    })
    .assert_success();
}
