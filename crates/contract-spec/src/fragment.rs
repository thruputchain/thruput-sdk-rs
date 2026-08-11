//! Owned raw-spec fragment types.
//!
//! `groundhogspecraw*` fragments are internal to the SDK toolchain: the
//! `#[contract]` macros emit them (as const views) and spec-tool parses and
//! canonicalizes them. Downstream consumers only ever see the finalized
//! `ContractSpec`, so the fragment types live here rather than in the
//! protocol-level `groundhog-spec` crate.
//!
//! These types are the raw wire format. Any change to their encoding — a new
//! field, a new variant, a reordering — must bump
//! [`RAW_SPEC_FORMAT`](crate::RAW_SPEC_FORMAT) and its mirrors; see
//! [`crate::format`]. The stream is unframed, so a consumer that disagrees
//! about the layout desynchronizes rather than failing cleanly.

use groundhog_spec::{
    ContractEventDef, ContractExceptionType, ContractSpecMethod,
    ContractSpecString, ContractTypeDef,
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
    pub throws: VecM<ContractExceptionType>,
    pub emits: VecM<ContractSpecString>,
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
pub struct SpecMethodFragment {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
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
