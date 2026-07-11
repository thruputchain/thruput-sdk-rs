use multi_root_storage::{Ledger, Registry};
use sdk::storage::{
    DomainRecord, Keypath, STORAGE_DOMAINS, check_duplicate,
    contract_state_root,
};

#[test]
fn both_roots_register_and_do_not_collide() {
    let records: &[DomainRecord] = &STORAGE_DOMAINS;
    // Records identify roots by type path (the framed domain bytes are
    // opaque outside the sdk crate).
    let type_paths: std::collections::HashSet<&str> =
        records.iter().map(|r| r.type_path).collect();
    assert!(type_paths.iter().any(|t| t.ends_with("::Ledger")));
    assert!(type_paths.iter().any(|t| t.ends_with("::Registry")));
    assert_eq!(check_duplicate(records), None);
}

#[test]
fn root_subtrees_are_disjoint() {
    let l = contract_state_root::<Ledger>();
    let r = contract_state_root::<Registry>();
    // Behavioral: each root is seeded with its ident as the first child
    // frame, so the two subtrees are disjoint.
    assert_eq!(
        l.path_bytes(),
        Keypath::default().child("Ledger").path_bytes()
    );
    assert_eq!(
        r.path_bytes(),
        Keypath::default().child("Registry").path_bytes()
    );
    assert_ne!(l.path_bytes(), r.path_bytes());
}
