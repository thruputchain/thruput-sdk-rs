// Unit-level check of the dedup logic the generated uniqueness test relies on:
// `check_duplicate` flags two roots that share a domain. The real link-level
// proof (a generated #[test] panicking on a genuine collision) lives in the
// workspace-excluded `multi_root_collision` crate plus its detection script.
use sdk::storage::{check_duplicate, DomainRecord};

#[test]
fn duplicate_domains_are_detected() {
    let records = [
        DomainRecord { domain: "Dup", type_path: "a::Dup" },
        DomainRecord { domain: "Dup", type_path: "b::Dup" },
    ];
    assert_eq!(check_duplicate(&records), Some((0, 1)));
}
