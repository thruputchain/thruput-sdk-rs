fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("entrypoint-id") => {
            groundhog_test_vectors::entrypoint_vectors::print()
        }
        Some("signed-op") => {
            groundhog_test_vectors::signed_op_vectors::print()
        }
        Some("xdr") => groundhog_test_vectors::xdr_vectors::print(),
        Some("domain-separator") => {
            groundhog_test_vectors::domain_separator_vectors::print()
        }
        _ => {
            eprintln!(
                "Usage: groundhog-test-vectors <entrypoint-id|xdr|domain-separator|signed-op>"
            );
            std::process::exit(1);
        }
    }
}
