//! `#[contract_type_spec(crate = "...")]` from a crate below `sdk`.
//!
//! This test crate deliberately has no `sdk` dependency — only
//! `groundhog-contract-spec` and `xdrs`, both of which `sdk` merely re-exports
//! as `__contract_spec` and `__xdrs`. That it compiles at all is the proof: a
//! crate that `sdk` depends on can derive its contract-spec impls instead of
//! hand-writing them, without forming a dependency cycle.
//!
//! The type mirrors `crates/u256`'s `U256`, the motivating consumer.

use groundhog_macros::ContractTypeSpec;

/// The shim a consuming crate provides. The derive only ever reaches for
/// `__contract_spec` and `__xdrs`, so matching those two names is the whole
/// contract.
#[doc(hidden)]
pub mod sdk_paths {
    pub use groundhog_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}

use groundhog_contract_spec as spec;

const LIMBS: usize = 8;

// The field exists to give the derive something to describe; nothing reads it.
#[allow(dead_code)]
#[derive(ContractTypeSpec)]
#[contract_type_spec(crate = "crate::sdk_paths")]
pub struct Derived([u32; LIMBS]);

// The derive references `ContractTypeName` but never emits it, so a consumer
// supplies it. No encoded bytes here, so nothing to drift.
impl spec::ContractTypeName for Derived {
    const NAME: &'static str = "Derived";
}

/// What the derive is expected to produce, written out by hand.
///
/// This is the assertion the brief calls for: the derived fragment is compared
/// against an independent literal rather than eyeballed, so a change to either
/// the derive's shape or the spec encoding fails here instead of silently
/// shipping an ABI break.
const EXPECTED_FRAGMENT: spec::SpecFragmentView<'static> =
    spec::SpecFragmentView::TypeDef(spec::ContractTypeDefView::Struct(
        spec::ContractStructDefView {
            name: "Derived",
            // A tuple struct's single unnamed field is `_0`, not the Rust-side
            // field name (there isn't one). Any consumer that wants a
            // meaningful spec field name needs a named-field struct.
            fields: &[spec::ContractFieldView {
                name: "_0",
                ty: spec::ContractTypeView::Array(
                    spec::ContractArrayTypeView {
                        element: &spec::ContractTypeView::U32,
                        len: LIMBS as u32,
                    },
                ),
            }],
        },
    ));

#[test]
fn derived_fragment_matches_hand_written_reference() {
    assert_eq!(__groundhog_spec_fragment_Derived, EXPECTED_FRAGMENT);
}

#[test]
fn derived_type_def_round_trips_through_the_shim_paths() {
    // Exercises the `ContractTypeSpec` / `ContractTypeRefSpec` impls the derive
    // emitted through `crate::sdk_paths`, not through `::sdk`.
    use spec::{ContractTypeRefSpec, ContractTypeSpec};

    assert_eq!(
        <Derived as ContractTypeSpec>::TYPE_DEF,
        match EXPECTED_FRAGMENT {
            spec::SpecFragmentView::TypeDef(def) => def,
            _ => unreachable!(),
        }
    );
    assert_eq!(
        <Derived as ContractTypeRefSpec>::INFO.ty,
        spec::ContractTypeView::Named("Derived")
    );
}

/// The link-section static the derive emits is `cfg`-gated to riscv32, so
/// on the host it never compiles. Encode the same fragment the same way here so
/// the `__xdrs` half of the shim is exercised and the wire bytes are pinned.
///
/// Deliberately spelled `sdk_paths::__xdrs` rather than `xdrs`: that is the name
/// the macro reaches for, so a shim missing it fails here on the host instead of
/// only under a guest target nothing in CI builds this crate for.
const RAW_LEN: usize = EXPECTED_FRAGMENT.const_txs_size();

const RAW_BYTES: [u8; RAW_LEN] = {
    let mut w = sdk_paths::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
    EXPECTED_FRAGMENT.const_write_txs(&mut w);
    w.into_bytes()
};

#[test]
fn raw_fragment_encoding_is_pinned() {
    assert_eq!(RAW_BYTES, EXPECTED_RAW_BYTES);
}

#[rustfmt::skip]
const EXPECTED_RAW_BYTES: [u8; RAW_LEN] = [
    0, 0, 0, 2, // SpecFragmentView::TypeDef
    0, 0, 0, 0, // ContractTypeDefView::Struct
    0, 0, 0, 7, b'D', b'e', b'r', b'i', b'v', b'e', b'd', 0, // name "Derived"
    0, 0, 0, 1, // one field
    0, 0, 0, 2, b'_', b'0', 0, 0, // field name "_0"
    0, 0, 0, 16, // ContractTypeView::Array
    0, 0, 0, 2, // element: ContractTypeView::U32
    0, 0, 0, 8, // len: 8
];
