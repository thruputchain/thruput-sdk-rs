//! Spec type identity: a `#[contract_type]` type names itself in the spec by
//! a fully qualified id — `CRATE@MAJOR::module::path::Name` — not by its bare
//! ident.
//!
//! A bare ident cannot tell two user-defined types apart: two crates, or two
//! modules of one crate, can each define a `Meta`, and a spec that names both
//! `Meta` either merges them or fails to build. The id is derived where the
//! type is declared (`module_path!()` plus the declaring crate's name and
//! major version), so it is unique by construction and travels cross-crate as
//! an associated const.
//!
//! The major version follows the same rule as interface versions
//! (`groundhog_contract_util::semver::major_version_from_string`): the
//! leftmost non-zero component, so `1.2.3` is `1` and `0.33.0` is `0.33`.
//! Two semver-compatible releases therefore share an id, and only a breaking
//! release changes it.

#![allow(unexpected_cfgs, dead_code)]

use groundhog_contract_util::semver::major_version_from_string;
use sdk::__contract_spec::{
    ContractTypeDefView, ContractTypeName, ContractTypeSpec, ContractTypeView,
};
use sdk::*;

/// The id prefix every type declared in this test crate must carry, derived
/// the same way the macro derives it rather than hardcoded, so a version bump
/// does not churn these tests.
fn crate_at_major() -> String {
    format!(
        "{}@{}",
        env!("CARGO_PKG_NAME"),
        major_version_from_string(env!("CARGO_PKG_VERSION"))
    )
}

#[contract_type]
pub struct Meta {
    pub count: u32,
}

mod inner {
    use sdk::*;

    /// Same ident as the root `Meta`, different module, different shape.
    #[contract_type]
    pub struct Meta {
        pub label: u64,
    }

    pub mod deeper {
        use sdk::*;

        #[contract_type]
        pub struct Meta {
            pub flag: bool,
        }
    }
}

#[contract_type(name = "SpecPayload")]
pub struct Payload {
    pub count: u32,
}

/// A type that references another user-defined type in a field.
#[contract_type]
pub struct Wrapper {
    pub meta: Meta,
    pub inner_meta: inner::Meta,
}

#[test]
fn id_is_crate_major_module_path_and_name() {
    assert_eq!(
        <Meta as ContractTypeName>::NAME,
        format!("{}::contract_type_ids::Meta", crate_at_major()),
    );
}

#[test]
fn id_carries_the_module_path_within_the_crate() {
    assert_eq!(
        <inner::Meta as ContractTypeName>::NAME,
        format!("{}::contract_type_ids::inner::Meta", crate_at_major()),
    );
    assert_eq!(
        <inner::deeper::Meta as ContractTypeName>::NAME,
        format!(
            "{}::contract_type_ids::inner::deeper::Meta",
            crate_at_major()
        ),
    );
}

#[test]
fn types_sharing_an_ident_in_different_modules_get_distinct_ids() {
    let ids = [
        <Meta as ContractTypeName>::NAME,
        <inner::Meta as ContractTypeName>::NAME,
        <inner::deeper::Meta as ContractTypeName>::NAME,
    ];
    let distinct: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(distinct.len(), ids.len(), "ids must be distinct: {ids:?}");
    // Every one still ends in the declared name, which is what the display
    // name is reduced from.
    for id in ids {
        assert!(id.ends_with("::Meta"), "{id}");
    }
}

#[test]
fn every_id_is_rooted_at_the_declaring_crate_and_major() {
    for id in [
        <Meta as ContractTypeName>::NAME,
        <inner::Meta as ContractTypeName>::NAME,
        <Payload as ContractTypeName>::NAME,
        <Wrapper as ContractTypeName>::NAME,
    ] {
        assert!(
            id.starts_with(&format!("{}::", crate_at_major())),
            "{id} is not rooted at {}",
            crate_at_major()
        );
    }
}

#[test]
fn custom_name_replaces_the_final_segment_only() {
    assert_eq!(
        <Payload as ContractTypeName>::NAME,
        format!("{}::contract_type_ids::SpecPayload", crate_at_major()),
    );
}

#[test]
fn the_type_def_names_itself_by_its_id() {
    assert_eq!(
        <Meta as ContractTypeSpec>::TYPE_DEF.symbol(),
        <Meta as ContractTypeName>::NAME,
    );
    assert_eq!(
        <inner::Meta as ContractTypeSpec>::TYPE_DEF.symbol(),
        <inner::Meta as ContractTypeName>::NAME,
    );
}

#[test]
fn a_field_reference_resolves_to_the_referenced_types_own_id() {
    let ContractTypeDefView::Struct(def) =
        <Wrapper as ContractTypeSpec>::TYPE_DEF
    else {
        panic!("Wrapper should be a struct def");
    };
    let named = |field: &str| -> String {
        let f = def
            .fields
            .iter()
            .find(|f| f.name == field)
            .unwrap_or_else(|| panic!("no field {field}"));
        match f.ty {
            ContractTypeView::Named(name) => name.to_string(),
            other => panic!("field {field} is not a named type: {other:?}"),
        }
    };
    // Two fields whose Rust types share an ident must reference different
    // spec types.
    assert_eq!(named("meta"), <Meta as ContractTypeName>::NAME);
    assert_eq!(named("inner_meta"), <inner::Meta as ContractTypeName>::NAME);
    assert_ne!(named("meta"), named("inner_meta"));
}
