#[test]
fn hasher_test() {
    sdk::client_test(
        |env| env.register_mock(hasher::HasherImpl::__register_mock),
        |c: &mut hasher::HasherClient| c.run(),
    );
}
