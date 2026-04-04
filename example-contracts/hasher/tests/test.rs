#[test]
fn hasher_test() {
    sdk::client_test(
        |env| hasher::__groundhog_register_mock_package(env),
        |c: &hasher::HasherClient| c.run(),
    );
}
