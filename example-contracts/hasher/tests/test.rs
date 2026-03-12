#[test]
fn hasher_test() {
    sdk::client_test(hasher::register, |c: &mut hasher::HasherClient| c.run());
}
