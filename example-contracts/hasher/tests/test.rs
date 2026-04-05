#[test]
fn hasher_test() {
    sdk::client_test(
        |env| hasher::register_mock_contract(env),
        |c: &hasher::HasherClient| c.run(),
    );
}
