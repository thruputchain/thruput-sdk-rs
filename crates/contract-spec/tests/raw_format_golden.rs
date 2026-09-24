//! Pins the raw spec wire format against an encoded sample.
//!
//! The `groundhogspecraw*` section is an unframed concatenation of XDR
//! `SpecFragment` values, so a producer and consumer that disagree about the
//! layout do not fail cleanly — the reader stops short of a field it does not
//! know, resumes mid-fragment, and decodes garbage from there on. That is
//! only safe if every encoding change is accompanied by a version bump, and
//! remembering to bump is exactly what failed once already: `emits` was added
//! to `SpecInterfaceFragment` while the section name stayed `v0`, so an older
//! CLI reading a newer contract reported a parse error with no hint that a
//! version skew was the cause.
//!
//! This test removes the need to remember. It encodes one value of every
//! `SpecFragment` variant and compares against a checked-in hex blob, so any
//! change to the fragment types — or to any type reachable from them — fails
//! here first.
//!
//! # When this test fails
//!
//! If you changed the raw spec encoding on purpose:
//!
//! 1. bump `RAW_SPEC_FORMAT` in `src/format.rs`,
//! 2. bump `RAW_SECTION_NAME` to match,
//! 3. bump `raw_spec_format` in `Cargo.toml`,
//! 4. bump the `link_section` literals in `groundhog-macros`,
//! 5. re-bless `GOLDEN` below from the `left` value this test prints.
//!
//! Steps 1-4 are each pinned by their own test, so a partial bump fails too.
//! Then tell the CLI: `groundhog-spec-tool`'s new version has to reach the
//! `groundhog` CLI before contracts built with it can be built there.
//!
//! If you did *not* change it on purpose, something you edited altered the
//! encoding transitively — most likely a type in `groundhog-spec`.

use groundhog_contract_spec::{
    ContractEventDef, ContractField, ContractSpecId, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    RAW_SPEC_FORMAT, SpecFragment, SpecInterfaceAnchor, SpecInterfaceFragment,
    SpecMethodFragment,
};
use xdrs::txs::encode;

/// Hex encoding of `sample_fragments()` under `RAW_SPEC_FORMAT`.
///
/// Re-bless only together with a format bump — see the module docs.
const GOLDEN: &str = include_str!("raw_format_golden.hex");

fn spec_string(s: &str) -> ContractSpecString {
    ContractSpecString::try_from(s).expect("spec string fits")
}

/// A spec id for the sample below. All four segments are populated, so the
/// golden pins the whole id encoding and not just its declared name.
/// The declarer id the sample interface fragments carry.
fn golden_declarer() -> ContractSpecId {
    ContractSpecId {
        crate_name: spec_string("golden_if"),
        major: spec_string("1"),
        module_path: spec_string("golden_if"),
        declared: spec_string("Iface"),
    }
}

fn spec_id(declared: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: spec_string("goldencrate"),
        major: spec_string("0"),
        module_path: spec_string("goldencrate::sample"),
        declared: spec_string(declared),
    }
}

fn field(name: &str, ty: ContractType) -> ContractField {
    ContractField {
        name: spec_string(name),
        ty,
    }
}

fn struct_def(name: &str) -> ContractTypeDef {
    ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: spec_id(name),
            fields: vec![field("a", ContractType::U32)]
                .try_into()
                .expect("field list fits"),
        },
    }
}

fn event_def(name: &str) -> ContractEventDef {
    ContractEventDef {
        name: spec_id(name),
        signature: [0xAB; 32].into(),
        topics: vec![field("from", ContractType::Address)]
            .try_into()
            .expect("topic list fits"),
        fields: vec![field("b", ContractType::U32)]
            .try_into()
            .expect("field list fits"),
    }
}

/// One value of every `SpecFragment` variant.
///
/// Every variant is present deliberately: a golden that skipped one would let
/// that variant's encoding change without failing here, which is the whole
/// failure this test exists to prevent.
fn sample_fragments() -> Vec<SpecFragment> {
    vec![
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: spec_string("Iface"),
                interface_version: spec_string("1.2.3"),
                declared_by: golden_declarer(),
                throws: vec![].try_into().expect("throws list fits"),
                emits: vec![spec_id("Ev")]
                    .try_into()
                    .expect("emits list fits"),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: spec_string("Iface"),
                interface_version: spec_string("1.2.3"),
                declared_by: golden_declarer(),
                method: ContractSpecMethod {
                    name: spec_string("do_thing"),
                    args: vec![field("arg", ContractType::U32)]
                        .try_into()
                        .expect("args fit"),
                    return_type: ContractType::U32,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: struct_def("Plain"),
        },
        SpecFragment::ExportedSpecTypeDef {
            type_def: struct_def("Exported"),
        },
        SpecFragment::SpecEvent {
            event_def: event_def("Ev"),
        },
        SpecFragment::ExportedSpecEvent {
            event_def: event_def("ExportedEv"),
        },
        SpecFragment::InterfaceAnchor {
            interface_anchor: SpecInterfaceAnchor {
                interface_name: spec_string("Iface"),
                interface_version: spec_string("1.2.3"),
                declared_by: golden_declarer(),
            },
        },
    ]
}

/// Encode the sample the way the macros do: bare concatenation, no framing.
fn encode_sample() -> String {
    let mut out = String::new();
    for fragment in sample_fragments() {
        for byte in encode(&fragment).expect("sample fragments must encode") {
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

#[test]
fn raw_spec_encoding_matches_golden() {
    let actual = encode_sample();
    assert_eq!(
        actual,
        GOLDEN.trim(),
        "\n\nThe raw spec wire format changed (RAW_SPEC_FORMAT is currently \
         {RAW_SPEC_FORMAT}).\n\nAn older groundhog CLI reading a contract \
         built with this SDK cannot detect the change unless the format \
         version is bumped: the fragment stream is unframed, so it \
         desynchronizes and reports an unrelated-looking parse error. If \
         this change is intentional, bump RAW_SPEC_FORMAT, RAW_SECTION_NAME, \
         Cargo.toml's raw_spec_format, and the link_section literals in \
         groundhog-macros, then re-bless tests/raw_format_golden.hex with \
         the `left` value above. See this file's module docs.\n\n",
    );
}

/// Guards the golden itself: a fixture that silently stopped covering some
/// variants would keep passing while protecting less than it claims.
#[test]
fn golden_covers_every_fragment_variant() {
    let fragments = sample_fragments();
    let mut discriminants: Vec<_> = fragments
        .iter()
        .map(|fragment| match fragment {
            SpecFragment::SpecInterface { .. } => 0,
            SpecFragment::SpecMethod { .. } => 1,
            SpecFragment::SpecTypeDef { .. } => 2,
            SpecFragment::ExportedSpecTypeDef { .. } => 3,
            SpecFragment::SpecEvent { .. } => 4,
            SpecFragment::ExportedSpecEvent { .. } => 5,
            SpecFragment::InterfaceAnchor { .. } => 6,
        })
        .collect();
    discriminants.sort_unstable();
    discriminants.dedup();
    assert_eq!(
        discriminants,
        (0..=6).collect::<Vec<i32>>(),
        "sample_fragments() must carry every SpecFragment variant; a new \
         variant needs a sample here (and a format bump)",
    );
}
