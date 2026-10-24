use crate::semver::major_version_from_string;
use crate::semver::try_major_version_from_string;

#[test]
fn try_major_version_accepts_triplets_and_rejects_the_rest() {
    assert_eq!(try_major_version_from_string("1.2.3").as_deref(), Some("1"));
    assert_eq!(
        try_major_version_from_string("0.9.2").as_deref(),
        Some("0.9")
    );
    assert_eq!(try_major_version_from_string("1"), None);
    assert_eq!(try_major_version_from_string("1.2"), None);
    assert_eq!(try_major_version_from_string("bogus"), None);
    assert_eq!(try_major_version_from_string(""), None);
    assert_eq!(try_major_version_from_string("1.2.3.4"), None);
}

#[test]
fn test_cargo_major_version_major_nonzero() {
    assert_eq!(major_version_from_string("1.2.3"), "1");
    assert_eq!(major_version_from_string("2.0.0"), "2");
    assert_eq!(major_version_from_string("10.5.99"), "10");
}

#[test]
fn test_cargo_major_version_minor_nonzero() {
    assert_eq!(major_version_from_string("0.9.2"), "0.9");
    assert_eq!(major_version_from_string("0.1.0"), "0.1");
    assert_eq!(major_version_from_string("0.15.3"), "0.15");
}

#[test]
fn test_cargo_major_version_patch_only() {
    assert_eq!(major_version_from_string("0.0.2"), "0.0.2");
    assert_eq!(major_version_from_string("0.0.1"), "0.0.1");
    assert_eq!(major_version_from_string("0.0.0"), "0.0.0");
}

#[test]
fn test_cargo_major_version_edge_cases() {
    // Single component
    assert_eq!(major_version_from_string("1.0.0-alpha.1"), "1");
    assert_eq!(major_version_from_string("0.0.1-alpha.1+build123"), "0.0.1");
    assert_eq!(major_version_from_string("0.0.0-beta.1"), "0.0.0-beta.1");
}

/// major_version_from_string should be idempotent: applying it to its
/// own output (re-parsed as a version) should yield the same string.
/// This matters because the error selector and entrypoint ID both
/// feed the result into a hash — if the function weren't stable,
/// the same version could produce different selectors.
#[test]
fn test_major_version_idempotent() {
    let cases = [
        "1.2.3", "2.0.0", "10.5.99", "0.9.2", "0.1.0", "0.15.3", "0.0.2",
        "0.0.1", "0.0.0",
    ];
    for version_str in cases {
        let first = major_version_from_string(version_str);
        // Pad the major version back to a parseable semver string
        let reparseable = match first.matches('.').count() {
            0 => format!("{}.0.0", first),
            1 => format!("{}.0", first),
            _ => first.clone(),
        };
        let second = major_version_from_string(&reparseable);
        assert_eq!(
            first, second,
            "not idempotent for input {}: first={}, reparsed as {}, second={}",
            version_str, first, reparseable, second
        );
    }
}

#[test]
fn test_exception_selector_deterministic() {
    let s1 = crate::exception_selector("1.2.3", "mycrate", "MyError");
    let s2 = crate::exception_selector("1.2.3", "mycrate", "MyError");
    assert_eq!(s1, s2);
}

#[test]
fn test_exception_selector_differs_by_version() {
    let s1 = crate::exception_selector("1.0.0", "mycrate", "MyError");
    let s2 = crate::exception_selector("2.0.0", "mycrate", "MyError");
    assert_ne!(s1, s2);
}

#[test]
fn test_exception_selector_differs_by_name() {
    let s1 = crate::exception_selector("1.0.0", "mycrate", "AlphaError");
    let s2 = crate::exception_selector("1.0.0", "mycrate", "BetaError");
    assert_ne!(s1, s2);
}

#[test]
fn test_exception_selector_same_major_same_selector() {
    // 1.2.3 and 1.9.0 have the same major version "1"
    let s1 = crate::exception_selector("1.2.3", "mycrate", "MyError");
    let s2 = crate::exception_selector("1.9.0", "mycrate", "MyError");
    assert_eq!(s1, s2);
}

#[test]
fn test_exception_selector_differs_by_crate() {
    let s1 = crate::exception_selector("1.0.0", "crate_a", "MyError");
    let s2 = crate::exception_selector("1.0.0", "crate_b", "MyError");
    assert_ne!(s1, s2);
}

#[test]
fn test_event_signature_deterministic() {
    let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    let s2 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    assert_eq!(s1, s2);
}

#[test]
fn test_event_signature_is_32_bytes() {
    let sig = crate::event_signature("1.0.0", "mycrate", "MyEvent");
    assert_eq!(sig.len(), 32);
}

#[test]
fn test_event_signature_same_major_same_signature() {
    let s1 = crate::event_signature("1.2.3", "mycrate", "MyEvent");
    let s2 = crate::event_signature("1.9.0", "mycrate", "MyEvent");
    assert_eq!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_version() {
    let s1 = crate::event_signature("1.0.0", "mycrate", "MyEvent");
    let s2 = crate::event_signature("2.0.0", "mycrate", "MyEvent");
    assert_ne!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_crate() {
    let s1 = crate::event_signature("1.0.0", "crate_a", "MyEvent");
    let s2 = crate::event_signature("1.0.0", "crate_b", "MyEvent");
    assert_ne!(s1, s2);
}

#[test]
fn test_event_signature_differs_by_name() {
    let s1 = crate::event_signature("1.0.0", "mycrate", "AlphaEvent");
    let s2 = crate::event_signature("1.0.0", "mycrate", "BetaEvent");
    assert_ne!(s1, s2);
}

#[test]
fn entrypoint_id_separators_prevent_field_boundary_collision() {
    use crate::{NormalizedMethodSignature, contract_entrypoint_id};
    // Without the `::` separators these two triples would both hash the
    // bytes "Foo12x" and collide to the same id (silent misrouting):
    //   interface "Foo1" + major "2"  + method "x"
    //   interface "Foo"  + major "12" + method "x"
    let a = contract_entrypoint_id(
        Some("Foo1"),
        "2.0.0",
        &NormalizedMethodSignature::new("x"),
    );
    let b = contract_entrypoint_id(
        Some("Foo"),
        "12.0.0",
        &NormalizedMethodSignature::new("x"),
    );
    assert_ne!(
        a, b,
        "distinct (interface, version, method) triples must produce \
             distinct entrypoint ids"
    );
}

#[test]
fn entrypoint_link_name_mirrors_id_preimage_equality() {
    use crate::{NormalizedMethodSignature, entrypoint_link_name};
    // The rv32 wrapper symbol carries the human-readable ID preimage so
    // duplicate-entrypoint link errors name the interface and method.
    // Equality of the link name must coincide exactly with equality of
    // the entrypoint ID: it is built from the same (interface, MAJOR
    // version, method-signature) triple — the full version must not
    // leak in, or two same-major crates would collide in dispatch while
    // their symbols differ.
    let name = |vers: &str| {
        entrypoint_link_name(
            Some("Ledger"),
            vers,
            &NormalizedMethodSignature::new("clash"),
        )
    };
    assert_eq!(name("0.4.1"), name("0.4.9"));
    assert_ne!(name("0.4.1"), name("0.5.0"));
    assert_eq!(name("0.4.1"), "__thruput_entrypoint::Ledger::0.4::clash");
}

#[test]
fn interface_id_is_deterministic_and_keyed_on_name_and_major() {
    use crate::interface_id;

    let a = interface_id(Some("ERC20"), "0.1.19");
    assert_eq!(a, interface_id(Some("ERC20"), "0.1.19"));
    // Same major line: same ID.
    assert_eq!(a, interface_id(Some("ERC20"), "0.1.0"));
    // Different major line, name, or the unnamed default: different ID.
    assert_ne!(a, interface_id(Some("ERC20"), "0.2.0"));
    assert_ne!(a, interface_id(Some("ERC20Metadata"), "0.1.19"));
    assert_ne!(a, interface_id(None, "0.1.19"));
    assert_eq!(
        interface_id(None, "0.1.19"),
        interface_id(Some(crate::UNNAMED_INTERFACE), "0.1.19")
    );
}

#[test]
fn interface_id_pinned() {
    // echo -n 'thruput-interface::ERC20::0.1' | shasum -a 256
    let expected = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(b"thruput-interface::ERC20::0.1");
        h.finalize()
    };
    assert_eq!(
        crate::interface_id(Some("ERC20"), "0.1.19")[..],
        expected[0..8]
    );
}
