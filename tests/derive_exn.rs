use sdk::exn::Exn;

#[derive(Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, sdk::Exn, thiserror::Error)]
#[repr(i32)]
enum TestError {
    #[error("a")]
    VariantA = 1,
    #[error("b")]
    VariantB(u64) = 2,
}

#[derive(Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, sdk::Exn, thiserror::Error)]
#[repr(i32)]
enum UnitOnlyError {
    #[error("first")]
    First = 1,
    #[error("second")]
    Second = 2,
}

#[derive(Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, sdk::Exn, thiserror::Error)]
#[error("named fields")]
struct NamedFieldsError {
    code: u32,
    value: u64,
}

#[test]
fn derive_exn_produces_stable_selector() {
    let s1 = TestError::SELECTOR;
    let s2 = TestError::SELECTOR;
    assert_eq!(s1, s2);
    assert_ne!(s1, 0);
}

#[test]
fn derive_exn_selector_matches_manual_sha256() {
    use sha2::{Digest, Sha256};
    // Canonical signature: "TestError::VariantA::VariantB(u64)"
    let sig = "TestError::VariantA::VariantB(u64)";
    let hash = Sha256::digest(sig.as_bytes());
    let expected = u64::from_be_bytes(hash[..8].try_into().unwrap());
    assert_eq!(TestError::SELECTOR, expected);
}

#[test]
fn derive_exn_unit_only_enum() {
    use sha2::{Digest, Sha256};
    let sig = "UnitOnlyError::First::Second";
    let hash = Sha256::digest(sig.as_bytes());
    let expected = u64::from_be_bytes(hash[..8].try_into().unwrap());
    assert_eq!(UnitOnlyError::SELECTOR, expected);
}

#[test]
fn derive_exn_named_fields_struct() {
    use sha2::{Digest, Sha256};
    let sig = "NamedFieldsError(u32,u64)";
    let hash = Sha256::digest(sig.as_bytes());
    let expected = u64::from_be_bytes(hash[..8].try_into().unwrap());
    assert_eq!(NamedFieldsError::SELECTOR, expected);
}

#[test]
fn derive_exn_different_types_have_different_selectors() {
    assert_ne!(TestError::SELECTOR, UnitOnlyError::SELECTOR);
    assert_ne!(TestError::SELECTOR, NamedFieldsError::SELECTOR);
    assert_ne!(UnitOnlyError::SELECTOR, NamedFieldsError::SELECTOR);
}
