use thruput_sdk::{CodeHash, ContractClient, DeployNonce, Named};

static NONCE: DeployNonce = DeployNonce::new(thruput_sdk::sha256_static!(
    "MULTI_IMPORTED_TRAITS_EXAMPLE_TEST_NONCE"
));

#[test]
fn package_registration_exposes_all_imported_interfaces() {
    let env = thruput_sdk::TestEnvironment::no_cost();
    let contract = multi_imported_traits::register_mock_contract(&env);
    let code = CodeHash::<(
        named_interface_callee::ExplicitNamedInterfaceClient,
        versioned_callee::VersionedCalleeClient,
    )>::of_mock(&contract);

    env.transact(|tx| {
        tx.call(&thruput_sdk::ContractAddress::named("test"))
            .run(move || {
                let addr = code.deploy(&NONCE);

                let named =
                    named_interface_callee::ExplicitNamedInterfaceClient::at(
                        &addr,
                    );
                let versioned =
                    versioned_callee::VersionedCalleeClient::at(&addr);

                assert_eq!(named.add_one(41), 42);
                assert_eq!(versioned.add_one(41), 43);
            })
    })
    .assert_success();
}
