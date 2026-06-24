// Two roots forced to the same domain must make the generated uniqueness
// test panic. We assert that by calling the checker directly against a
// hand-built record set (the generated #[test] uses the same function).
use sdk::storage::{check_duplicate, DomainRecord};

#[test]
fn duplicate_domains_are_detected() {
    let records = [
        DomainRecord { domain: "Dup", type_path: "a::Dup" },
        DomainRecord { domain: "Dup", type_path: "b::Dup" },
    ];
    assert_eq!(check_duplicate(&records), Some((0, 1)));
}
