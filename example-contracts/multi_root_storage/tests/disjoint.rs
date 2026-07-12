use multi_root_storage::{Ledger, Registry};
use sdk::storage::{
    DomainRecord, STORAGE_DOMAINS, check_duplicate, contract_state_root,
};

#[test]
fn both_roots_register_and_do_not_collide() {
    // Reference both state types so their distributed-slice registrations are
    // retained by the test link.
    let _ = contract_state_root::<Ledger>();
    let _ = contract_state_root::<Registry>();
    let records: &[DomainRecord] = &STORAGE_DOMAINS;
    // Records identify roots by type path (the framed domain bytes are
    // opaque outside the sdk crate).
    let type_paths: std::collections::HashSet<&str> =
        records.iter().map(|r| r.type_path).collect();
    assert!(type_paths.iter().any(|t| t.ends_with("::Ledger")));
    assert!(type_paths.iter().any(|t| t.ends_with("::Registry")));
    assert_eq!(check_duplicate(records), None);
}
