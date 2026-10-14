fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("entrypoint-id") => {
            thruput_test_vectors::entrypoint_vectors::print()
        }
        Some("signed-op") => thruput_test_vectors::signed_op_vectors::print(),
        Some("pubkey-auth") => {
            thruput_test_vectors::pubkey_auth_vectors::print()
        }
        Some("xdr") => thruput_test_vectors::xdr_vectors::print(),
        Some("domain-separator") => {
            thruput_test_vectors::domain_separator_vectors::print()
        }
        _ => {
            eprintln!(
                "Usage: thruput-test-vectors <entrypoint-id|xdr|domain-separator|signed-op|pubkey-auth>"
            );
            std::process::exit(1);
        }
    }
}
