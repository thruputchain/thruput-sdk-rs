mod common;
test_cfg!();

use sdk::exn::Exn;

#[derive(
    Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, thiserror::Error,
)]
#[sdk::exn]
#[repr(i32)]
enum TestError {
    #[error("a")]
    VariantA = 1,
    #[error("b")]
    VariantB(u64) = 2,
}

#[derive(
    Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, thiserror::Error,
)]
#[sdk::exn]
#[repr(i32)]
enum UnitOnlyError {
    #[error("first")]
    First = 1,
    #[error("second")]
    Second = 2,
}

#[derive(
    Debug, Clone, PartialEq, sdk::ReadXDR, sdk::WriteXDR, thiserror::Error,
)]
#[sdk::exn]
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
fn derive_exn_selector_matches_contract_util() {
    let expected = groundhog_contract_util::exception_selector(
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_NAME"),
        "TestError",
    );
    assert_eq!(TestError::SELECTOR, expected);
}

#[test]
fn derive_exn_different_types_have_different_selectors() {
    assert_ne!(TestError::SELECTOR, UnitOnlyError::SELECTOR);
    assert_ne!(TestError::SELECTOR, NamedFieldsError::SELECTOR);
    assert_ne!(UnitOnlyError::SELECTOR, NamedFieldsError::SELECTOR);
}
