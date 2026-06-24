use sdk::storage::{
    check_duplicate, contract_state_root, DomainRecord, StorageDomain,
    STORAGE_DOMAINS,
};
use multi_root_storage::{Ledger, Registry};

#[test]
fn both_roots_register_and_do_not_collide() {
    let records: &[DomainRecord] = &STORAGE_DOMAINS;
    let domains: std::collections::HashSet<&str> =
        records.iter().map(|r| r.domain).collect();
    assert!(domains.contains("Ledger"));
    assert!(domains.contains("Registry"));
    assert_eq!(check_duplicate(records), None);
}

#[test]
fn root_subtrees_are_disjoint() {
    assert_eq!(<Ledger as StorageDomain>::DOMAIN, "Ledger");
    assert_eq!(<Registry as StorageDomain>::DOMAIN, "Registry");
    let l = contract_state_root::<Ledger>();
    let r = contract_state_root::<Registry>();
    assert!(!l.keypath().addresses_same_slot(r.keypath()));
}
