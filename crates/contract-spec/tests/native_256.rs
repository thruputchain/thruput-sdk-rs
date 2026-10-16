use thruput_contract_spec::{ContractType, ContractTypeView};

#[test]
fn u256_view_converts_to_the_native_owned_type() {
    assert_eq!(
        ContractType::from(ContractTypeView::U256),
        ContractType::U256
    );
}

#[test]
fn i256_view_converts_to_the_native_owned_type() {
    assert_eq!(
        ContractType::from(ContractTypeView::I256),
        ContractType::I256
    );
}

#[test]
fn u256_view_encodes_with_protocol_discriminant() {
    assert_eq!(
        xdrs::txs::encode(&ContractTypeView::U256).unwrap(),
        [0, 0, 0, 18]
    );
}

#[test]
fn i256_view_encodes_with_protocol_discriminant() {
    assert_eq!(
        xdrs::txs::encode(&ContractTypeView::I256).unwrap(),
        [0, 0, 0, 19]
    );
}
