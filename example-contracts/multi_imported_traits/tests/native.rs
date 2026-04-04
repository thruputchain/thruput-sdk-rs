static NONCE: sdk::SHA256Hash =
    sdk::sha256_static!("MULTI_IMPORTED_TRAITS_EXAMPLE_TEST_NONCE");

#[test]
fn package_registration_exposes_all_imported_interfaces() {
    let env = sdk::TestEnvironment::no_cost();
    let contract =
        multi_imported_traits::__groundhog_register_mock_package(&env);
    let contract_hash = contract.as_hash().clone();

    sdk::contract_exec(env, move || {
        let addr = sdk::deploy_contract(&NONCE, &contract_hash);

        let named = named_interface_callee::ExplicitNamedInterfaceClient::new(
            &addr,
        );
        let versioned = versioned_callee::VersionedCalleeClient::new(&addr);

        assert_eq!(named.add_one(41), 42);
        assert_eq!(versioned.add_one(41), 43);
    })
    .unwrap_or_else(|e| {
        panic!("Multi imported traits package registration test failed: {e}")
    });
}
