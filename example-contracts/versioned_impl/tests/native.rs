use versioned_interface::AdderClient;

#[test]
fn imported_interface_keeps_its_version() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.2.0");
    assert_eq!(AdderClient::INTERFACE_VERSION, "0.1.0");

    sdk::client_test(
        |env| versioned_impl::register_mock_contract(env),
        |client: &AdderClient| assert_eq!(client.add_one(41), 42),
    );
}
