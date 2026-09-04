//! Pins the native contract-spec types for the SDK's 256-bit integers.

use groundhog_contract_spec::{ContractTypeRefSpec, ContractTypeView};
use groundhog_u256::{I256, U256};

#[test]
fn u256_uses_the_native_contract_spec_type() {
    assert_eq!(
        <U256 as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::U256
    );
}

#[test]
fn i256_uses_the_native_contract_spec_type() {
    assert_eq!(
        <I256 as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::I256
    );
}
