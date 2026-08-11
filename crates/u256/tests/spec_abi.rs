//! Pins `U256`'s contract-spec ABI. These bytes are wire-visible, so a change
//! here is breaking, not a refactor.
//!
//! The spec impls come from `#[derive(ContractTypeSpec)]` (PR #337); the
//! literals below were captured from the hand-written consts that preceded it,
//! which is what makes that swap verifiable. Guards the field name in
//! particular: the derive would call an unnamed field `_0`.

use groundhog_contract_spec as spec;
use groundhog_u256::U256;

/// What the derive is expected to produce, written out independently.
const EXPECTED_FRAGMENT: spec::SpecFragmentView<'static> =
    spec::SpecFragmentView::TypeDef(spec::ContractTypeDefView::Struct(
        spec::ContractStructDefView {
            name: "U256",
            fields: &[spec::ContractFieldView {
                name: "limbs",
                ty: spec::ContractTypeView::Array(
                    spec::ContractArrayTypeView {
                        element: &spec::ContractTypeView::U32,
                        len: 8,
                    },
                ),
            }],
        },
    ));

#[test]
fn derived_fragment_matches_the_hand_written_reference() {
    assert_eq!(
        groundhog_u256::__groundhog_spec_fragment_U256,
        EXPECTED_FRAGMENT
    );
}

#[test]
fn type_def_and_type_ref_are_unchanged() {
    use spec::{ContractTypeName, ContractTypeRefSpec, ContractTypeSpec};

    assert_eq!(<U256 as ContractTypeName>::NAME, "U256");
    assert_eq!(
        <U256 as ContractTypeRefSpec>::INFO.ty,
        spec::ContractTypeView::Named("U256")
    );
    assert_eq!(
        <U256 as ContractTypeSpec>::TYPE_DEF,
        match EXPECTED_FRAGMENT {
            spec::SpecFragmentView::TypeDef(def) => def,
            _ => unreachable!(),
        }
    );
}

const RAW_LEN: usize =
    groundhog_u256::__groundhog_spec_fragment_U256.const_txs_size();

const RAW_BYTES: [u8; RAW_LEN] = {
    let mut w = xdrs::txs::ConstWriter::<RAW_LEN>::new();
    groundhog_u256::__groundhog_spec_fragment_U256.const_write_txs(&mut w);
    w.into_bytes()
};

/// Captured from the hand-written consts that preceded the derive.
#[rustfmt::skip]
const EXPECTED_RAW_BYTES: [u8; 44] = [
    0, 0, 0, 2,                             // SpecFragmentView::TypeDef
    0, 0, 0, 0,                             // ContractTypeDefView::Struct
    0, 0, 0, 4, b'U', b'2', b'5', b'6',     // name "U256"
    0, 0, 0, 1,                             // one field
    0, 0, 0, 5, b'l', b'i', b'm', b'b', b's', 0, 0, 0, // field name "limbs"
    0, 0, 0, 16,                            // ContractTypeView::Array
    0, 0, 0, 2,                             // element: U32
    0, 0, 0, 8,                             // len: 8
];

#[test]
fn raw_fragment_encoding_is_pinned() {
    assert_eq!(RAW_LEN, EXPECTED_RAW_BYTES.len());
    assert_eq!(RAW_BYTES, EXPECTED_RAW_BYTES);
}
