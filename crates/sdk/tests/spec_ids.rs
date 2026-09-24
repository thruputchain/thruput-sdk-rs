//! Spec identity: every user-defined thing the spec names — a
//! `#[contract_type]`, a `#[contract_exception]`, a `#[derive(Event)]` — names
//! itself by a fully qualified id -- declaring crate, major version, module
//! path and declared name -- not by its bare ident.
//!
//! The segments travel apart rather than joined into one string. `name =
//! "..."` is substituted verbatim and may carry `::`, so a joined id could
//! not be split back into module path and declared name; see
//! `a_custom_name_carrying_a_path_is_taken_verbatim` for the case that
//! forces it.
//!
//! A bare ident cannot tell two of them apart: two crates, or two modules of
//! one crate, can each declare a `Meta`, and a spec that names both `Meta`
//! either merges them or fails to build. The id is derived where the item is
//! declared, from `module_path!()` and the declaring crate's major version, so
//! it is unique by construction and travels cross-crate as an associated
//! const.
//!
//! All three go through one code path (`common::spec_id::spec_id_tokens`), so
//! a type and an event declared side by side cannot drift apart.
//!
//! The major version follows the same rule as interface versions
//! (`groundhog_contract_util::semver::major_version_from_string`): the
//! leftmost non-zero component, so `1.2.3` is `1` and `0.33.0` is `0.33`.
//! Two semver-compatible releases therefore share an id, and only a breaking
//! release changes it.

#![allow(unexpected_cfgs, dead_code)]

use groundhog_contract_util::semver::major_version_from_string;
use sdk::__contract_spec::{
    ContractEventSpec, ContractTypeDefView, ContractTypeName,
    ContractTypeSpec, ContractTypeView,
};
use sdk::*;

/// The id prefix every type declared in this test crate must carry, derived
/// the same way the macro derives it rather than hardcoded, so a version bump
/// does not churn these tests.
///
/// The crate segment comes from `module_path!()` and the version from the
/// package being compiled. In a library crate those are the same crate; in an
/// integration test the module path is rooted at the test binary (`spec_ids`)
/// while the version is `sdk`'s, which is why this is built rather than
/// spelled out.
fn expected_crate() -> &'static str {
    env!("CARGO_PKG_NAME")
}

fn expected_major() -> String {
    major_version_from_string(env!("CARGO_PKG_VERSION"))
}

/// Assert an id segment by segment.
///
/// `module_suffix` is relative to this test binary's module root; empty means
/// the root itself. Note the crate appears twice over, spelled two ways: as
/// the package in `crate_name`, and as the Rust path root inside
/// `module_path` -- which in an integration test is the test binary, not the
/// library.
#[track_caller]
fn assert_id(
    id: sdk::__contract_spec::ContractSpecIdView<'static>,
    module_suffix: &str,
    declared: &str,
) {
    assert_eq!(id.crate_name, expected_crate(), "crate segment");
    assert_eq!(id.major, expected_major(), "major segment");
    let expected_module = if module_suffix.is_empty() {
        module_path!().to_string()
    } else {
        format!("{}::{}", module_path!(), module_suffix)
    };
    assert_eq!(id.module_path, expected_module, "module path segment");
    assert_eq!(id.declared, declared, "declared segment");
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

/// A name is substituted verbatim, `::` and all — nothing rejects one that
/// spells extra segments.
///
/// It cannot forge an id outside this crate: the crate name and major version
/// prefix every id, and both come from the declaring crate. So the most an
/// author can do is collide with another of their own types, in a crate they
/// control, which is their conflict to resolve — canonicalization still
/// refuses two shapes under one id.
#[contract_type(name = "spelled::Out")]
pub struct SpelledOut {
    pub v: u32,
}

/// A type that references another user-defined type in a field.
#[contract_type]
pub struct Wrapper {
    pub meta: Meta,
    pub inner_meta: inner::Meta,
}

#[test]
fn id_is_crate_major_module_path_and_name() {
    assert_id(<Meta as ContractTypeName>::NAME, "", "Meta");
}

#[test]
fn id_carries_the_module_path_within_the_crate() {
    assert_id(<inner::Meta as ContractTypeName>::NAME, "inner", "Meta");
    assert_id(
        <inner::deeper::Meta as ContractTypeName>::NAME,
        "inner::deeper",
        "Meta",
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
    // Only the module path tells them apart: each still declares `Meta`,
    // which is what a display name is reduced to.
    for id in ids {
        assert_eq!(id.declared, "Meta", "{id:?}");
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
        assert_eq!(id.crate_name, expected_crate(), "{id:?}");
        assert_eq!(id.major, expected_major(), "{id:?}");
    }
}

#[test]
fn custom_name_replaces_the_final_segment_only() {
    assert_id(<Payload as ContractTypeName>::NAME, "", "SpecPayload");
}

#[test]
fn a_custom_name_carrying_a_path_is_taken_verbatim() {
    // The whole value lands in `declared`, `::` and all, and the module path
    // is untouched. Joined into one string these two segments would be
    // indistinguishable from a type `Out` declared in a module `spelled` --
    // which is precisely why they are not joined.
    assert_id(<SpelledOut as ContractTypeName>::NAME, "", "spelled::Out");
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
    let named = |field: &str| {
        let f = def
            .fields
            .iter()
            .find(|f| f.name == field)
            .unwrap_or_else(|| panic!("no field {field}"));
        match f.ty {
            ContractTypeView::Named(name) => name,
            other => panic!("field {field} is not a named type: {other:?}"),
        }
    };
    // Two fields whose Rust types share an ident must reference different
    // spec types.
    assert_eq!(named("meta"), <Meta as ContractTypeName>::NAME);
    assert_eq!(named("inner_meta"), <inner::Meta as ContractTypeName>::NAME);
    assert_ne!(named("meta"), named("inner_meta"));
}

// --- Events -----------------------------------------------------------------
//
// An event is a user-defined name in the spec exactly as a type is, and
// collides exactly as a type would, so it carries the same id. An interface's
// `emits(...)` entry reads the event's own `EVENT_DEF.name`, so the reference
// follows the event rather than repeating its ident.

#[derive(Event)]
pub struct Moved {
    #[topic]
    pub who: ContractAddress,
    pub amount: u64,
}

mod evt {
    use sdk::*;

    /// Same ident as the root `Moved`, different module, different payload.
    #[derive(Event)]
    pub struct Moved {
        #[topic]
        pub who: ContractAddress,
        pub label: u32,
    }
}

#[test]
fn an_event_names_itself_by_its_id() {
    assert_id(<Moved as ContractEventSpec>::EVENT_DEF.name, "", "Moved");
    assert_id(
        <evt::Moved as ContractEventSpec>::EVENT_DEF.name,
        "evt",
        "Moved",
    );
}

#[test]
fn events_sharing_an_ident_in_different_modules_get_distinct_ids() {
    assert_ne!(
        <Moved as ContractEventSpec>::EVENT_DEF.name,
        <evt::Moved as ContractEventSpec>::EVENT_DEF.name,
    );
}

#[test]
fn an_event_and_a_type_declared_alike_agree_on_the_id_shape() {
    // The point of the shared code path: both are rooted the same way and end
    // in their own declared name.
    let type_id = <Meta as ContractTypeName>::NAME;
    let event_id = <Moved as ContractEventSpec>::EVENT_DEF.name;
    assert_eq!(type_id.crate_name, event_id.crate_name);
    assert_eq!(type_id.major, event_id.major);
    assert_eq!(type_id.module_path, event_id.module_path);
    assert_eq!(type_id.declared, "Meta");
    assert_eq!(event_id.declared, "Moved");
}

#[test]
fn an_events_signature_is_not_the_id_and_does_not_move_with_it() {
    // The signature is topic 0 on every emitted event — on-wire, and read by
    // deployed indexers — so qualifying the spec name deliberately leaves it
    // alone. It is hashed from (crate, major version, ident).
    //
    // A consequence worth seeing rather than discovering: because the ident is
    // all it carries of the type, two events with the same ident in one crate
    // share a signature even though their ids now distinguish them. Giving the
    // signature the module too would be an on-wire break, so it is deliberately
    // not part of this change.
    assert_eq!(
        Moved::SIGNATURE,
        evt::Moved::SIGNATURE,
        "signatures are ident-derived; this pins that qualifying ids did not \
         move them",
    );
    assert_ne!(
        <Moved as ContractEventSpec>::EVENT_DEF.name,
        <evt::Moved as ContractEventSpec>::EVENT_DEF.name,
    );
}

// --- The one way left to forge an id ------------------------------------------

#[contract_type]
pub struct Colliding {
    pub limit: u32,
}

/// Claims `Colliding`'s id: same crate, same module, and `name` supplies the
/// same final segment.
#[contract_type(name = "Colliding")]
pub struct Decoy {
    pub flag: u64,
}

#[test]
fn two_declarations_in_one_module_can_claim_one_id() {
    // The path segments are derived, so the final segment is the only part an
    // author picks — and pointing two declarations at one name is the only way
    // left to make two defs answer to a single id. It is not rejected at the
    // macro (each declaration is locally fine, and a macro cannot see its
    // sibling); it is caught downstream, where two shapes under one id are
    // ambiguous. `spec-tool`'s `different_shapes_under_one_id_are_still_rejected`
    // covers that, and the `typedef_id_collision` fixture covers it end to end.
    //
    // This pins the premise those rest on: that these two really do collide.
    assert_eq!(
        <Colliding as ContractTypeName>::NAME,
        <Decoy as ContractTypeName>::NAME,
    );
}

// --- A declared identity version pins the id too --------------------------

/// `#[contract_exception(version = "...")]` pins the version the selector is
/// derived from, so a released crate can keep its selector still. The id has
/// to be pinned with it, or the two identities of one type drift apart: the
/// selector holding at the declared version while the id tracks the crate's.
#[contract_exception(version = "2.0.0")]
pub enum PinnedError {
    Denied = 1,
}

#[contract_exception]
pub enum UnpinnedError {
    Denied = 1,
}

#[test]
fn a_declared_version_pins_the_ids_major_segment() {
    let id = <PinnedError as ContractTypeName>::NAME;
    assert_eq!(id.major, "2", "the declared version pins the major");
    assert_ne!(id.major, expected_major(), "and it is not the crate's");
    assert_eq!(id.declared, "PinnedError");
}

#[test]
fn without_a_declared_version_the_id_takes_the_crates() {
    assert_id(
        <UnpinnedError as ContractTypeName>::NAME,
        "",
        "UnpinnedError",
    );
}
