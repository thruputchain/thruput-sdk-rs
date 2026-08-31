//! Pins `U256`'s and `I256`'s contract-spec ABI. These bytes are
//! wire-visible, so a change here is breaking, not a refactor.
//!
//! The spec impls come from `#[derive(ContractTypeSpec)]` (PR #337); the
//! literals below were captured from the hand-written consts that preceded it,
//! which is what makes that swap verifiable. Guards the field name in
//! particular: the derive would call an unnamed field `_0`.

use groundhog_contract_spec as spec;
use groundhog_u256::{I256, U256};

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

// ---------------------------------------------------------------------------
// I256
// ---------------------------------------------------------------------------
//
// Deliberately the same shape as `U256`: eight `u32` limbs, holding the two's
// complement bits. Only the type name distinguishes them on the wire, and that
// is enough -- a decoder that confuses the two would read the same 32 bytes
// with the wrong sign, which is exactly what a distinct name prevents.

/// What the derive is expected to produce for `I256`, written out
/// independently.
const EXPECTED_I256_FRAGMENT: spec::SpecFragmentView<'static> =
    spec::SpecFragmentView::TypeDef(spec::ContractTypeDefView::Struct(
        spec::ContractStructDefView {
            name: "I256",
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
fn derived_i256_fragment_matches_the_hand_written_reference() {
    assert_eq!(
        groundhog_u256::__groundhog_spec_fragment_I256,
        EXPECTED_I256_FRAGMENT
    );
}

#[test]
fn i256_type_def_and_type_ref_are_pinned() {
    use spec::{ContractTypeName, ContractTypeRefSpec, ContractTypeSpec};

    assert_eq!(<I256 as ContractTypeName>::NAME, "I256");
    assert_eq!(
        <I256 as ContractTypeRefSpec>::INFO.ty,
        spec::ContractTypeView::Named("I256")
    );
    assert_eq!(
        <I256 as ContractTypeSpec>::TYPE_DEF,
        match EXPECTED_I256_FRAGMENT {
            spec::SpecFragmentView::TypeDef(def) => def,
            _ => unreachable!(),
        }
    );
    // The two are distinct spec types, not aliases of one another.
    assert_ne!(
        <I256 as ContractTypeName>::NAME,
        <U256 as ContractTypeName>::NAME
    );
}

const I256_RAW_LEN: usize =
    groundhog_u256::__groundhog_spec_fragment_I256.const_txs_size();

const I256_RAW_BYTES: [u8; I256_RAW_LEN] = {
    let mut w = xdrs::txs::ConstWriter::<I256_RAW_LEN>::new();
    groundhog_u256::__groundhog_spec_fragment_I256.const_write_txs(&mut w);
    w.into_bytes()
};

/// `U256`'s pinned encoding with one byte changed -- the type name's first
/// character. Written out in full anyway, so a structural change cannot hide
/// behind a shared constant.
#[rustfmt::skip]
const EXPECTED_I256_RAW_BYTES: [u8; 44] = [
    0, 0, 0, 2,                             // SpecFragmentView::TypeDef
    0, 0, 0, 0,                             // ContractTypeDefView::Struct
    0, 0, 0, 4, b'I', b'2', b'5', b'6',     // name "I256"
    0, 0, 0, 1,                             // one field
    0, 0, 0, 5, b'l', b'i', b'm', b'b', b's', 0, 0, 0, // field name "limbs"
    0, 0, 0, 16,                            // ContractTypeView::Array
    0, 0, 0, 2,                             // element: U32
    0, 0, 0, 8,                             // len: 8
];

#[test]
fn i256_raw_fragment_encoding_is_pinned() {
    assert_eq!(I256_RAW_LEN, EXPECTED_I256_RAW_BYTES.len());
    assert_eq!(I256_RAW_BYTES, EXPECTED_I256_RAW_BYTES);
    // Same length as `U256`'s, and differing in exactly the name byte.
    assert_eq!(I256_RAW_LEN, RAW_LEN);
    let differing = I256_RAW_BYTES
        .iter()
        .zip(RAW_BYTES.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differing, 1);
}
