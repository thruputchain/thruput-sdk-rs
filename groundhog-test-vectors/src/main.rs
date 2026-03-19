fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("entrypoint-id") => groundhog_test_vectors::entrypoint_vectors::print(),
        Some("xdr") => groundhog_test_vectors::xdr_vectors::print(),
        _ => {
            eprintln!("Usage: groundhog-test-vectors <entrypoint-id|xdr>");
            std::process::exit(1);
        }
    }
}
