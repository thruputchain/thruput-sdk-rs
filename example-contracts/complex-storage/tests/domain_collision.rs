// Unit-level check of the dedup logic the generated uniqueness test relies on:
// `check_duplicate` flags two roots that share a domain. The real link-level
// proof (a generated #[test] panicking on a genuine collision) lives in the
// workspace-excluded `multi_root_collision` crate plus its detection script.
use sdk::storage::{
    DomainRecord, FramedDomain, check_duplicate, frame_domain,
    framed_domain_len,
};

#[test]
fn duplicate_domains_are_detected() {
    const N: usize = framed_domain_len("Dup");
    const DUP: [u8; N] = frame_domain::<N>("Dup");
    let records = [
        DomainRecord {
            domain: FramedDomain::new(&DUP),
            type_path: "a::Dup",
        },
        DomainRecord {
            domain: FramedDomain::new(&DUP),
            type_path: "b::Dup",
        },
    ];
    assert_eq!(check_duplicate(&records), Some((0, 1)));
}
