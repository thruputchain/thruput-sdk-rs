#[test]
fn hasher_test() {
    sdk::client_test(hasher::test_setup::register, |c: &mut hasher::HasherClient| c.run());
}
