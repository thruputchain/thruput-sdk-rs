#[test]
fn hasher_test() {
    sdk::client_test(
        |env| env.register_mock::<hasher::HasherImpl, hasher::HasherImpl>(),
        |c: &mut hasher::HasherClient| c.run(),
    );
}
