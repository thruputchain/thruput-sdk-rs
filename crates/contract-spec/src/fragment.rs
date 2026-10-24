//! Owned raw-spec fragment types.
//!
//! `thruputspecraw*` fragments are internal to the SDK toolchain: the
//! `#[contract]` macros emit them (as const views) and spec-tool parses and
//! canonicalizes them. Downstream consumers only ever see the finalized
//! `ContractSpec`, so the fragment types live here rather than in the
//! protocol-level `thruput-spec` crate.
//!
//! These types are the raw wire format. Any change to their encoding — a new
//! field, a new variant, a reordering — must bump
//! [`RAW_SPEC_FORMAT`](crate::RAW_SPEC_FORMAT) and its mirrors; see
//! [`crate::format`]. The stream is unframed, so a consumer that disagrees
//! about the layout desynchronizes rather than failing cleanly.

use thruput_spec::{
    ContractEventDef, ContractExceptionType, ContractSpecId,
    ContractSpecMethod, ContractSpecString, ContractTypeDef,
};
use xdrs::*;

#[derive(
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
pub struct SpecInterfaceFragment {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
    /// The item that declares this interface: the `#[contract]` trait's spec
    /// id (`CRATE@MAJOR::module::path::Trait`), or `CRATE@MAJOR` for a bare
    /// impl or free-function contract. See [`SpecInterfaceAnchor::declared_by`].
    pub declared_by: ContractSpecId,
    pub throws: VecM<ContractExceptionType>,
    pub emits: VecM<ContractSpecId>,
}

#[derive(
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
/// A method belonging to `interface_name`@`interface_version`.
///
/// Emitted only by a site that also anchors that interface — a `#[contract]`
/// impl, or `export_interface_spec!` in a carrier. A `#[contract]` trait
/// definition emits its [`SpecInterfaceFragment`] as a candidate but no
/// methods: two unrelated traits can collide on one interface key (default
/// name = trait ident, default version = crate version), and candidate
/// methods from a trait nothing implements would otherwise publish as
/// phantom methods of the contract that merely links them. (`declared_by`
/// now separates such traits in spec-tool as well.)
pub struct SpecMethodFragment {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
    /// The item that declares this interface: the `#[contract]` trait's spec
    /// id (`CRATE@MAJOR::module::path::Trait`), or `CRATE@MAJOR` for a bare
    /// impl or free-function contract. See [`SpecInterfaceAnchor::declared_by`].
    pub declared_by: ContractSpecId,
    pub method: ContractSpecMethod,
}

#[derive(
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
pub struct SpecInterfaceAnchor {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
    /// The item that declares the anchored interface: the `#[contract]`
    /// trait's spec id (`CRATE@MAJOR::module::path::Trait`), or `CRATE@MAJOR`
    /// for a bare impl or free-function contract.
    ///
    /// The interface *key* is (name, version) because that is all the wire
    /// carries — `interface_id` and every entrypoint id hash the bare name
    /// and major version, so that independent crates can implement one
    /// interface. Two unrelated traits can therefore share a key. This field
    /// is spec-only and tells them apart: spec-tool keeps only the fragments
    /// of the declarer an artifact anchors, and rejects an artifact that
    /// anchors two declarers under one key, since nothing on the wire could
    /// tell their interfaces apart.
    pub declared_by: ContractSpecId,
}

#[derive(
    ReadXDR,
    WriteXDR,
    ConstXDRSize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
#[repr(i32)]
pub enum SpecFragment {
    SpecInterface {
        interface_fragment: SpecInterfaceFragment,
    } = 0,
    SpecMethod {
        method_fragment: SpecMethodFragment,
    } = 1,
    SpecTypeDef {
        type_def: ContractTypeDef,
    } = 2,
    ExportedSpecTypeDef {
        type_def: ContractTypeDef,
    } = 3,
    SpecEvent {
        event_def: ContractEventDef,
    } = 4,
    ExportedSpecEvent {
        event_def: ContractEventDef,
    } = 5,
    InterfaceAnchor {
        interface_anchor: SpecInterfaceAnchor,
    } = 6,
}
