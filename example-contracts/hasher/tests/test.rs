#[test]
fn hasher_test() {
    sdk::client_test(
        |env| hasher::register_mock_contract(env),
        |c: &hasher::HasherClient| c.run(),
    );
}

/// Exercises `groundhog-domain-separator`'s SHA256 syscall backend from inside
/// a contract.
#[test]
fn pubkey_account_address_test() {
    sdk::client_test(
        |env| hasher::register_mock_contract(env),
        |c: &hasher::HasherClient| c.check_pubkey_account_address(),
    );
}
