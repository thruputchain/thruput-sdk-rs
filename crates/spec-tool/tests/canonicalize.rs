use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractExceptionType,
    ContractField, ContractSpec, ContractSpecId, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractUnionCase, ContractUnionCaseType, ContractUnionDef,
    ContractVectorType, SpecFragment, SpecInterfaceAnchor,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec, canonicalize_spec_with_exports,
    parse_raw_fragments,
};
use xdrs::txs::encode;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

/// A spec id for a test fixture: only `declared` varies, so a fixture reads
/// the way it used to while still carrying a well-formed id.
/// How a fixture id renders in a diagnostic, matching groundhog-spec's
/// `Display` for `ContractSpecId`. Spelled out here rather than rendered
/// through it so a change to the rendering has to be made deliberately in both
/// places.
fn id_str(declared: &str) -> String {
    format!("testcrate::{declared} (testcrate@0)")
}

/// A declarer id: the trait `declared` in crate `krate`. Declarers differ by
/// crate here, which is what the collision fixtures turn on.
/// How a declarer renders in a diagnostic, matching groundhog-spec's `Display`
/// for `ContractSpecId`.
fn declarer_str(krate: &str, declared: &str) -> String {
    format!("{krate}::{declared} ({krate}@0.1)")
}

/// An id that names no declarer at all. `declared` being empty is legitimate
/// (that is the crate declaring its own interface), so it is the crate segment
/// that has to be missing for there to be no declarer.
fn no_declarer() -> ContractSpecId {
    ContractSpecId {
        crate_name: s(""),
        major: s(""),
        module_path: s(""),
        declared: s(""),
    }
}

fn declarer(krate: &str, declared: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: s(krate),
        major: s("0.1"),
        module_path: s(krate),
        declared: s(declared),
    }
}

fn id(value: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: s("testcrate"),
        major: s("0"),
        module_path: s("testcrate"),
        declared: s(value),
    }
}

/// Canonicalize, first anchoring every interface the fragments mention — which
/// is what a real `#[contract] impl` emits (an `InterfaceAnchor`) and what spec
/// shaking requires. These fixtures hand-build fragments and bypass the macro,
/// so we add the anchors here to mirror a real artifact.
fn canon(fragments: &[SpecFragment]) -> Result<ContractSpec, SpecToolError> {
    let mut v: Vec<SpecFragment> = fragments.to_vec();
    let mut keys = std::collections::BTreeSet::new();
    for f in fragments {
        match f {
            SpecFragment::SpecInterface {
                interface_fragment: i,
            } => {
                keys.insert((
                    i.interface_name.clone(),
                    i.interface_version.clone(),
                    i.declared_by.clone(),
                ));
            }
            SpecFragment::SpecMethod { method_fragment: m } => {
                keys.insert((
                    m.interface_name.clone(),
                    m.interface_version.clone(),
                    m.declared_by.clone(),
                ));
            }
            _ => {}
        }
    }
    for (interface_name, interface_version, declared_by) in keys {
        v.push(SpecFragment::InterfaceAnchor {
            interface_anchor: SpecInterfaceAnchor {
                interface_name,
                interface_version,
                declared_by,
            },
        });
    }
    canonicalize_spec(&v)
}

fn exn(ty: ContractType) -> ContractExceptionType {
    ContractExceptionType { selector: 0, ty }
}

fn spec_str(value: &ContractSpecString) -> &str {
    std::str::from_utf8(value).unwrap()
}

#[test]
fn canonicalize_spec_drops_unanchored_interfaces() {
    // Strict shaking: an interface candidate with no `InterfaceAnchor` (this
    // calls `canonicalize_spec` directly, not the auto-anchoring `canon`
    // helper) is dropped — there is intentionally no "include all" fallback.
    // With nothing anchored or exported, the whole spec shakes to nothing,
    // which reports as NoSpecFragments: the caller (rewrite) treats it as
    // "no spec section" for raw contracts and as a hard error for modules
    // with no entrypoints (the missing-#[contract] guard).
    let err = canonicalize_spec(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }])
    .unwrap_err();
    assert!(matches!(err, SpecToolError::NoSpecFragments), "{err}");
}

#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canon(&[
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![ContractField {
                        name: s("request"),
                        ty: ContractType::Named {
                            named: id("Payload"),
                        },
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Named {
                        named: id("Response"),
                    },
                },
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("pong"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("DemoError"),
                    variants: vec![ContractEnumVariant {
                        name: s("Nope"),
                        value: 7,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("Payload"),
                    fields: vec![ContractField {
                        name: s("count"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("Response"),
                    fields: vec![ContractField {
                        name: s("ok"),
                        ty: ContractType::Bool,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec_str(&spec.interfaces[0].name), "Named");
    assert_eq!(spec_str(&spec.interfaces[0].vers), "0.1.0");
    assert_eq!(
        spec.interfaces[0]
            .methods
            .iter()
            .map(|method| spec_str(&method.name))
            .collect::<Vec<_>>(),
        vec!["ping", "pong"]
    );
    assert_eq!(
        spec.type_defs
            .iter()
            .map(|type_def| match type_def {
                ContractTypeDef::TypeDefStruct { struct_def } =>
                    spec_str(&struct_def.name.declared),
                ContractTypeDef::TypeDefEnum { enum_def } =>
                    spec_str(&enum_def.name.declared),
                ContractTypeDef::TypeDefUnion { union_def } =>
                    spec_str(&union_def.name.declared),
                _ => panic!("unexpected variant"),
            })
            .collect::<Vec<_>>(),
        vec!["DemoError", "Payload", "Response"]
    );
}

#[test]
fn canonicalize_spec_merges_repeated_interface_buckets_when_throws_agree() {
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("zeta"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("alpha"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("DemoError"),
                    variants: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(
        spec.interfaces[0]
            .methods
            .iter()
            .map(|method| spec_str(&method.name))
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}

#[test]
fn canonicalize_spec_dedups_identical_methods() {
    let method = ContractSpecMethod {
        name: s("alpha"),
        args: vec![ContractField {
            name: s("value"),
            ty: ContractType::U32,
        }]
        .try_into()
        .unwrap(),
        return_type: ContractType::Void,
    };

    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: method.clone(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method,
            },
        },
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec.interfaces[0].methods.len(), 1);
    assert_eq!(spec_str(&spec.interfaces[0].methods[0].name), "alpha");
}

#[test]
fn canonicalize_spec_rejects_conflicting_methods_with_same_name() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("alpha"),
                    args: vec![ContractField {
                        name: s("value"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("alpha"),
                    args: vec![ContractField {
                        name: s("value"),
                        ty: ContractType::I32,
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingMethod(name) if name == "Named@0.1.0::alpha")
    );
}

#[test]
fn canonicalize_spec_dedups_identical_type_defs() {
    let payload = ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: id("Payload"),
            fields: vec![ContractField {
                name: s("count"),
                ty: ContractType::U32,
            }]
            .try_into()
            .unwrap(),
        },
    };
    let response = ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: id("Response"),
            fields: vec![ContractField {
                name: s("payload"),
                ty: ContractType::Named {
                    named: id("Payload"),
                },
            }]
            .try_into()
            .unwrap(),
        },
    };

    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: id("Response"),
                    },
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: payload.clone(),
        },
        SpecFragment::SpecTypeDef { type_def: response },
        SpecFragment::SpecTypeDef { type_def: payload },
    ])
    .unwrap();

    assert_eq!(
        spec.type_defs
            .iter()
            .map(|type_def| match type_def {
                ContractTypeDef::TypeDefStruct { struct_def } =>
                    spec_str(&struct_def.name.declared),
                ContractTypeDef::TypeDefEnum { enum_def } =>
                    spec_str(&enum_def.name.declared),
                ContractTypeDef::TypeDefUnion { union_def } =>
                    spec_str(&union_def.name.declared),
                _ => panic!("unexpected variant"),
            })
            .collect::<Vec<_>>(),
        vec!["Payload", "Response"]
    );
}

#[test]
fn canonicalize_spec_rejects_missing_named_type_defs() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::MissingTypeDef(name) if name == id_str("DemoError"))
    );
}

#[test]
fn canonicalize_spec_rejects_method_only_interface_buckets() {
    let err = canon(&[SpecFragment::SpecMethod {
        method_fragment: SpecMethodFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            method: ContractSpecMethod {
                name: s("ping"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::Void,
            },
        },
    }])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::MissingInterfaceFragment(key) if key == "Named@0.1.0")
    );
}

#[test]
fn canonicalize_spec_rejects_conflicting_type_defs() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("DemoError"),
                    variants: vec![ContractEnumVariant {
                        name: s("Nope"),
                        value: 7,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("DemoError"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingTypeDef(name) if name == id_str("DemoError"))
    );
}

#[test]
fn canonicalize_spec_shakes_unreachable_conflicting_type_defs() {
    // Deliberate semantic change (matches event-def policy): a def conflict
    // is only an error if the symbol is reachable from the retained spec.
    // Unreachable conflicting defs are all shaken; none may appear in the
    // output and the conflict must not fail the build.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("NeverUsed"),
                    variants: vec![ContractEnumVariant {
                        name: s("Left"),
                        value: 1,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("NeverUsed"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap();

    assert!(
        spec.type_defs.is_empty(),
        "unreachable conflicting defs must be shaken, got {:?}",
        spec.type_defs
    );
}

#[test]
fn conflicting_defs_ignored_when_type_is_unreachable() {
    // A caller links two versions of an interface crate (candidates MarketV1
    // and MarketV2) to call other contracts, implementing neither. Each
    // version defines a differently-shaped type named "SharedConfig". Both
    // candidates are shaken, so the def conflict must not fail the caller's
    // build, and neither def may appear in the final spec.
    let market = |name: &str| SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    };
    let configure = |name: &str| SpecFragment::SpecMethod {
        method_fragment: SpecMethodFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            method: ContractSpecMethod {
                name: s("configure"),
                args: vec![ContractField {
                    name: s("cfg"),
                    ty: ContractType::Named {
                        named: id("SharedConfig"),
                    },
                }]
                .try_into()
                .unwrap(),
                return_type: ContractType::Void,
            },
        },
    };
    let spec = canonicalize_spec(&[
        iface_with_emits("Caller", &[]),
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Caller"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        market("MarketV1"),
        configure("MarketV1"),
        market("MarketV2"),
        configure("MarketV2"),
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("SharedConfig"),
                    fields: vec![ContractField {
                        name: s("limit"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("SharedConfig"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
        anchor("Caller"),
    ])
    .unwrap();
    assert!(
        spec.type_defs.is_empty(),
        "unreachable conflicting defs must be shaken, got {:?}",
        spec.type_defs
    );
    let ifaces: Vec<&str> =
        spec.interfaces.iter().map(|i| spec_str(&i.name)).collect();
    assert_eq!(ifaces, vec!["Caller"]);
}

#[test]
fn canonicalize_spec_rejects_conflicting_interface_throws() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("OtherError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("DemoError"),
                    variants: vec![].try_into().unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("OtherError"),
                    variants: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceThrows(key) if key == "Named@0.1.0")
    );
}

#[test]
fn parse_raw_fragments_decodes_concatenated_xdr_stream() {
    let fragments = vec![
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(ContractType::Named {
                    named: id("DemoError"),
                })]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![ContractField {
                        name: s("value"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: id("DemoError"),
                    variants: vec![ContractEnumVariant {
                        name: s("Nope"),
                        value: 7,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ];

    let mut bytes = Vec::new();
    for fragment in &fragments {
        bytes.extend(encode(fragment).unwrap());
    }

    assert_eq!(parse_raw_fragments(&bytes).unwrap(), fragments);
}

#[test]
fn canonicalize_spec_rejects_non_semver_interface_versions() {
    let err = canon(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("v1"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }])
    .unwrap_err();

    assert_eq!(
        err,
        SpecToolError::InvalidRawFragment(
            "interface version must be a semver triplet like \"1.0.0\" \
             (optionally with -PRERELEASE and +BUILD suffixes), got v1"
                .into()
        )
    );
}

#[test]
fn canonicalize_spec_accepts_prerelease_interface_versions() {
    // The macro-side validator accepts `version = "1.0.0-rc.1"` (semver
    // pre-release/build suffixes); ingestion must agree, or a contract with
    // a pre-release version compiles and then fails at spec rewrite.
    let spec = canon(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("1.0.0-rc.1"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }])
    .unwrap();
    assert_eq!(spec_str(&spec.interfaces[0].vers), "1.0.0-rc.1");
}

#[test]
fn canonicalize_spec_rejects_non_canonical_semver_versions() {
    // These pass the digits-only shape check but are not canonical semver;
    // the strict major-version parser (used to compute entrypoint IDs
    // during method shaking) panics on them, so they must be rejected at
    // ingestion — crafted or corrupt raw sections must produce
    // SpecToolError, never a panic.
    for bad in ["01.0.0", "0.00.1", "99999999999999999999.0.0"] {
        let err = canon(&[SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s(bad),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        }])
        .unwrap_err();
        assert!(
            matches!(err, SpecToolError::InvalidRawFragment(_)),
            "{bad}: {err}"
        );
    }
}

#[test]
fn canonicalize_spec_resolves_types_in_multi_field_union_cases() {
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("get"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: id("Envelope"),
                    },
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefUnion {
                union_def: ContractUnionDef {
                    name: id("Envelope"),
                    cases: vec![
                        ContractUnionCase {
                            name: s("Empty"),
                            discriminant: 0,
                            case_type: ContractUnionCaseType::UnionCaseVoid,
                        },
                        ContractUnionCase {
                            name: s("Full"),
                            discriminant: 1,
                            case_type:
                                ContractUnionCaseType::UnionCaseStruct {
                                    struct_fields: vec![
                                        ContractField {
                                            name: s("payload"),
                                            ty: ContractType::Named {
                                                named: id("Payload"),
                                            },
                                        },
                                        ContractField {
                                            name: s("count"),
                                            ty: ContractType::U32,
                                        },
                                    ]
                                    .try_into()
                                    .unwrap(),
                                },
                        },
                        ContractUnionCase {
                            name: s("Pair"),
                            discriminant: 2,
                            case_type: ContractUnionCaseType::UnionCaseTuple {
                                tuple: vec![
                                    ContractType::Named {
                                        named: id("Payload"),
                                    },
                                    ContractType::U32,
                                ]
                                .try_into()
                                .unwrap(),
                            },
                        },
                    ]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("Payload"),
                    fields: vec![ContractField {
                        name: s("data"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let type_names: Vec<_> = spec
        .type_defs
        .iter()
        .map(|td| match td {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name.declared)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name.declared)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name.declared)
            }
            _ => panic!("unexpected variant"),
        })
        .collect();
    assert!(type_names.contains(&"Envelope"));
    assert!(type_names.contains(&"Payload"));
}

#[test]
fn canonicalize_spec_exported_type_is_included_while_unreachable_regular_is_not()
 {
    // ExportedTypeDef should be included regardless of reachability.
    // A plain SpecTypeDef that is unreachable from method signatures should
    // NOT be included.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        // This type is exported explicitly — must appear in output.
        SpecFragment::ExportedSpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("ExportedEvent"),
                    fields: vec![ContractField {
                        name: s("value"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        // This type is a plain typedef unreachable from any signature — must
        // NOT appear in output.
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("NeverUsedHelper"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let type_names: Vec<_> = spec
        .type_defs
        .iter()
        .map(|td| match td {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name.declared)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name.declared)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name.declared)
            }
            _ => panic!("unexpected variant"),
        })
        .collect();

    assert!(
        type_names.contains(&"ExportedEvent"),
        "exported type should be present"
    );
    assert!(
        !type_names.contains(&"NeverUsedHelper"),
        "unreachable plain type should be absent"
    );
}

#[test]
fn canonicalize_spec_exported_type_pulls_in_transitive_deps() {
    // When an exported type references another named type, that dependency
    // must also be resolved and included in the output.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        // ExportedEvent references Metadata as a field type.
        SpecFragment::ExportedSpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("ExportedEvent"),
                    fields: vec![ContractField {
                        name: s("meta"),
                        ty: ContractType::Named {
                            named: id("Metadata"),
                        },
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        // Metadata is a transitive dep — not exported directly.
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("Metadata"),
                    fields: vec![ContractField {
                        name: s("tag"),
                        ty: ContractType::U64,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let type_names: Vec<_> = spec
        .type_defs
        .iter()
        .map(|td| match td {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name.declared)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name.declared)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name.declared)
            }
            _ => panic!("unexpected variant"),
        })
        .collect();

    assert!(
        type_names.contains(&"ExportedEvent"),
        "exported type should be present"
    );
    assert!(
        type_names.contains(&"Metadata"),
        "transitive dep of exported type should be present"
    );
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

use groundhog_contract_spec::ContractEventDef;

fn base_interface_fragments() -> Vec<SpecFragment> {
    vec![
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ]
}

/// A distinct signature per event name.
///
/// A real event's signature is `sha256(MAJOR :: CRATE :: Ident)`, so two
/// differently-named events have different ones. Canonicalization now rejects
/// two retained events that share a signature, so a fixture handing every
/// event the same placeholder would read as a collision.
fn signature_for(name: &str) -> [u8; 32] {
    let mut sig = [0u8; 32];
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, byte) in name.bytes().enumerate() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        sig[i % 32] ^= (h >> 24) as u8;
    }
    sig[31] ^= name.len() as u8;
    sig
}

fn event_def(name: &str, payload_ty: ContractType) -> ContractEventDef {
    ContractEventDef {
        name: id(name),
        signature: signature_for(name).into(),
        topics: vec![ContractField {
            name: s("from"),
            ty: ContractType::Address,
        }]
        .try_into()
        .unwrap(),
        fields: vec![ContractField {
            name: s("amount"),
            ty: payload_ty,
        }]
        .try_into()
        .unwrap(),
    }
}

#[test]
fn canonicalize_spec_collects_and_dedupes_event_defs() {
    let mut fragments = base_interface_fragments();
    // ExportedSpecEvent is always retained; identical duplicates (e.g. from
    // another codegen unit) are deduped regardless of fragment variant.
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def("TransferEvent", ContractType::U128),
    });
    // An identical duplicate merges.
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def("TransferEvent", ContractType::U128),
    });
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def("ApprovalEvent", ContractType::I128),
    });

    let spec = canonicalize_spec(&fragments).unwrap();

    let names: Vec<_> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, vec!["ApprovalEvent", "TransferEvent"]);
}

#[test]
fn canonicalize_spec_resolves_type_defs_referenced_by_events() {
    // ExportedSpecEvent is retained regardless of interface emits; its field
    // types are resolved transitively.
    let mut fragments = base_interface_fragments();
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def(
            "MintEvent",
            ContractType::Named {
                named: id("Payload"),
            },
        ),
    });
    fragments.push(SpecFragment::SpecTypeDef {
        type_def: ContractTypeDef::TypeDefStruct {
            struct_def: ContractStructDef {
                name: id("Payload"),
                fields: vec![ContractField {
                    name: s("count"),
                    ty: ContractType::U32,
                }]
                .try_into()
                .unwrap(),
            },
        },
    });

    let spec = canonicalize_spec(&fragments).unwrap();

    assert_eq!(spec.events.len(), 1);
    assert_eq!(spec.type_defs.len(), 1);

    // Without the typedef fragment the named reference must fail.
    let mut missing = base_interface_fragments();
    missing.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def(
            "MintEvent",
            ContractType::Named {
                named: id("Payload"),
            },
        ),
    });
    assert_eq!(
        canonicalize_spec(&missing),
        Err(SpecToolError::MissingTypeDef(id_str("Payload")))
    );
}

#[test]
fn canonicalize_spec_rejects_conflicting_event_defs() {
    // Use ExportedSpecEvent so both defs are retained; conflict check must fire.
    let mut fragments = base_interface_fragments();
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def("TransferEvent", ContractType::U128),
    });
    fragments.push(SpecFragment::ExportedSpecEvent {
        event_def: event_def("TransferEvent", ContractType::I128),
    });

    assert_eq!(
        canonicalize_spec(&fragments),
        Err(SpecToolError::ConflictingEventDef(id_str("TransferEvent")))
    );
}

#[test]
fn canonicalize_spec_rejects_event_with_too_many_topics() {
    let mut fragments = base_interface_fragments();
    let too_many_topics: Vec<ContractField> = (0..3)
        .map(|i| ContractField {
            name: s(&format!("topic{i}")),
            ty: ContractType::Address,
        })
        .collect();
    fragments.push(SpecFragment::SpecEvent {
        event_def: ContractEventDef {
            name: id("NoisyEvent"),
            signature: [0xAB; 32].into(),
            topics: too_many_topics.try_into().unwrap(),
            fields: vec![].try_into().unwrap(),
        },
    });

    assert!(matches!(
        canonicalize_spec(&fragments),
        Err(SpecToolError::InvalidRawFragment(_))
    ));
}

#[test]
fn canonicalize_spec_rejects_event_without_name() {
    let mut fragments = base_interface_fragments();
    fragments.push(SpecFragment::SpecEvent {
        event_def: ContractEventDef {
            name: id(""),
            signature: [0xAB; 32].into(),
            topics: vec![].try_into().unwrap(),
            fields: vec![].try_into().unwrap(),
        },
    });

    assert!(matches!(
        canonicalize_spec(&fragments),
        Err(SpecToolError::InvalidRawFragment(_))
    ));
}

#[test]
fn non_utf8_interface_name_is_rejected_not_panicked() {
    let result = canon(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: ContractSpecString::try_from(vec![
                0xFFu8, 0xFE, 0xFD,
            ])
            .unwrap(),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }]);

    assert!(matches!(result, Err(SpecToolError::InvalidRawFragment(_))));
}

#[test]
fn non_utf8_type_name_is_rejected_not_panicked() {
    let result = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: ContractSpecId {
                            crate_name: s("testcrate"),
                            major: s("0"),
                            module_path: s("testcrate"),
                            declared: ContractSpecString::try_from(vec![
                                0xC0u8, 0x80,
                            ])
                            .unwrap(),
                        },
                    },
                },
            },
        },
    ]);

    assert!(matches!(result, Err(SpecToolError::InvalidRawFragment(_))));
}

#[test]
fn duplicate_exception_selector_with_different_types_is_rejected() {
    let result = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::Named { named: id("ErrA") },
                    },
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::Named { named: id("ErrB") },
                    },
                ]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ]);

    assert!(matches!(
        result,
        Err(SpecToolError::ConflictingExceptionSelector(_))
    ));
}

#[test]
fn identical_duplicate_exception_entries_are_deduped() {
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::U32,
                    },
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::U32,
                    },
                ]
                .try_into()
                .unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ])
    .unwrap();

    assert_eq!(spec.interfaces[0].throws.len(), 1);
}

// ---------------------------------------------------------------------------
// Event shaking
// ---------------------------------------------------------------------------

fn event(name: &str) -> ContractEventDef {
    ContractEventDef {
        name: id(name),
        signature: signature_for(name).into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    }
}

fn iface_with_emits(name: &str, emits: &[&str]) -> SpecFragment {
    SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            throws: vec![].try_into().unwrap(),
            emits: emits
                .iter()
                .map(|e| id(e))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        },
    }
}

fn anchor(name: &str) -> SpecFragment {
    SpecFragment::InterfaceAnchor {
        interface_anchor: SpecInterfaceAnchor {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
        },
    }
}

#[test]
fn events_of_unanchored_interfaces_are_shaken() {
    // Impl'd interface A emits AEvent; interface B is an unimplemented
    // candidate from the same interface crate emitting BEvent. Only AEvent
    // may survive.
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &["AEvent"]),
        SpecFragment::SpecEvent {
            event_def: event("AEvent"),
        },
        iface_with_emits("B", &["BEvent"]),
        SpecFragment::SpecEvent {
            event_def: event("BEvent"),
        },
        anchor("A"),
    ])
    .unwrap();
    let names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, vec!["AEvent"]);
}

#[test]
fn exported_events_survive_without_any_interface_reference() {
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        SpecFragment::ExportedSpecEvent {
            event_def: event("LooseEvent"),
        },
        anchor("A"),
    ])
    .unwrap();
    let names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, vec!["LooseEvent"]);
}

#[test]
fn conflicting_defs_ignored_when_event_is_shaken() {
    // B's conflicting def of "Ev" must not fail A's build once B is shaken.
    let mut other = event("Ev");
    other.signature = [1u8; 32].into();
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        iface_with_emits("B", &["Ev"]),
        SpecFragment::SpecEvent {
            event_def: event("Ev"),
        },
        SpecFragment::SpecEvent { event_def: other },
        anchor("A"),
    ])
    .unwrap();
    assert!(spec.events.is_empty());
}

#[test]
fn conflicting_defs_of_retained_event_still_error() {
    let mut other = event("Ev");
    other.signature = [1u8; 32].into();
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["Ev"]),
        SpecFragment::SpecEvent {
            event_def: event("Ev"),
        },
        SpecFragment::SpecEvent { event_def: other },
        anchor("A"),
    ])
    .unwrap_err();
    assert!(
        matches!(err, SpecToolError::ConflictingEventDef(_)),
        "{err}"
    );
}

#[test]
fn emits_reference_without_def_errors() {
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["GhostEvent"]),
        anchor("A"),
    ])
    .unwrap_err();
    assert!(matches!(err, SpecToolError::MissingEventDef(_)), "{err}");
}

#[test]
fn conflicting_interface_emits_error() {
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["Y"]),
        iface_with_emits("A", &["X"]),
        SpecFragment::SpecEvent {
            event_def: event("X"),
        },
        SpecFragment::SpecEvent {
            event_def: event("Y"),
        },
        anchor("A"),
    ])
    .unwrap_err();
    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceEmits(_)),
        "{err}"
    );
}

/// Interface fragment with the given throws list and empty emits.
fn iface_with_throws(
    name: &str,
    throws: &[ContractExceptionType],
) -> SpecFragment {
    SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            declared_by: declarer("test_if", "Iface"),
            throws: throws.to_vec().try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }
}

// Interface-metadata conflicts (throws / emits / exception selectors) are
// only errors for anchored interfaces. Two unrelated interface crates can
// collide on the same interface key — the name defaults to the bare trait
// ident and the version to the crate version — so a caller linking both
// (implementing neither) must not fail on their disagreement, mirroring the
// typedef/event conflict policy.

#[test]
fn conflicting_throws_ignored_when_interface_is_unanchored() {
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        iface_with_throws(
            "B",
            &[exn(ContractType::Named { named: id("ErrX") })],
        ),
        iface_with_throws(
            "B",
            &[exn(ContractType::Named { named: id("ErrY") })],
        ),
        anchor("A"),
    ])
    .unwrap();
    let ifaces: Vec<&str> =
        spec.interfaces.iter().map(|i| spec_str(&i.name)).collect();
    assert_eq!(ifaces, vec!["A"]);
}

#[test]
fn conflicting_emits_ignored_when_interface_is_unanchored() {
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        iface_with_emits("B", &["X"]),
        iface_with_emits("B", &["Y"]),
        SpecFragment::SpecEvent {
            event_def: event("X"),
        },
        SpecFragment::SpecEvent {
            event_def: event("Y"),
        },
        anchor("A"),
    ])
    .unwrap();
    let ifaces: Vec<&str> =
        spec.interfaces.iter().map(|i| spec_str(&i.name)).collect();
    assert_eq!(ifaces, vec!["A"]);
    assert!(spec.events.is_empty());
}

#[test]
fn selector_conflict_ignored_when_interface_is_unanchored() {
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        iface_with_throws(
            "B",
            &[
                ContractExceptionType {
                    selector: 7,
                    ty: ContractType::Named { named: id("ErrA") },
                },
                ContractExceptionType {
                    selector: 7,
                    ty: ContractType::Named { named: id("ErrB") },
                },
            ],
        ),
        anchor("A"),
    ])
    .unwrap();
    let ifaces: Vec<&str> =
        spec.interfaces.iter().map(|i| spec_str(&i.name)).collect();
    assert_eq!(ifaces, vec!["A"]);
}

// ---------------------------------------------------------------------------
// Method shaking by export cross-reference
// ---------------------------------------------------------------------------

/// Export-section name of the entrypoint for `iface_name@vers::method`,
/// computed with the same contract-util function the shaker uses.
fn export_name(iface_name: &str, vers: &str, method: &str) -> String {
    groundhog_contract_util::entrypoint_export_name_for(
        Some(iface_name),
        vers,
        &groundhog_contract_util::NormalizedMethodSignature::new(method),
    )
}

fn ledger_fixture() -> Vec<SpecFragment> {
    // Colliding-key scenario: the contract implements crate A's `Ledger`
    // (method `ping`), and links crate B whose unrelated `Ledger` candidate
    // (same default name + version, identical empty metadata) contributes
    // method `steal` with a named payload type. Only `ping` has an exported
    // entrypoint.
    vec![
        iface_with_emits("Ledger", &[]),
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Ledger"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Ledger"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("steal"),
                    args: vec![ContractField {
                        name: s("cfg"),
                        ty: ContractType::Named {
                            named: id("LootCfg"),
                        },
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("LootCfg"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
        anchor("Ledger"),
    ]
}

#[test]
fn phantom_methods_without_exported_entrypoints_are_shaken() {
    let exports: std::collections::BTreeSet<String> =
        [export_name("Ledger", "0.1.0", "ping")]
            .into_iter()
            .collect();
    let spec =
        canonicalize_spec_with_exports(&ledger_fixture(), &exports).unwrap();
    let methods: Vec<&str> = spec.interfaces[0]
        .methods
        .iter()
        .map(|m| spec_str(&m.name))
        .collect();
    assert_eq!(methods, vec!["ping"]);
    assert!(
        spec.type_defs.is_empty(),
        "typedefs reachable only through shaken methods must drop, got {:?}",
        spec.type_defs
    );
}

#[test]
fn empty_export_set_disables_method_shaking() {
    // A spec-carrier binary (export_interface_spec!) exports no functions
    // at all; its interfaces must publish their full method list.
    let exports = std::collections::BTreeSet::new();
    let spec =
        canonicalize_spec_with_exports(&ledger_fixture(), &exports).unwrap();
    let methods: Vec<&str> = spec.interfaces[0]
        .methods
        .iter()
        .map(|m| spec_str(&m.name))
        .collect();
    assert_eq!(methods, vec!["ping", "steal"]);
    assert_eq!(spec.type_defs.len(), 1);
}

#[test]
fn phantom_methods_shaken_even_when_no_interface_method_is_exported() {
    // The carrier exemption is module-wide (zero exported functions), NOT
    // per-interface: a contract module (it exports SOMETHING) anchoring an
    // interface none of whose methods are exported — e.g. an implemented
    // empty marker trait whose key a collider crate squats with method
    // candidates — must still shake the phantoms rather than publish them.
    let exports: std::collections::BTreeSet<String> =
        ["unrelated_export".to_string()].into_iter().collect();
    let spec =
        canonicalize_spec_with_exports(&ledger_fixture(), &exports).unwrap();
    assert!(
        spec.interfaces[0].methods.is_empty(),
        "phantoms must shake even with zero real methods, got {:?}",
        spec.interfaces[0].methods
    );
    assert!(spec.type_defs.is_empty());
}

#[test]
fn canonicalize_spec_without_exports_does_not_shake_methods() {
    let spec = canonicalize_spec(&ledger_fixture()).unwrap();
    assert_eq!(spec.interfaces[0].methods.len(), 2);
}

// ---------------------------------------------------------------------------
// Signed tokens
// ---------------------------------------------------------------------------

fn signed_token(preimage_type: ContractType) -> ContractType {
    ContractType::SignedToken {
        signed_token_preimage:
            groundhog_contract_spec::ContractSignedTokenPreimage {
                preimage_type: Some(Box::new(preimage_type)),
            },
    }
}

fn opaque_signed_token() -> ContractType {
    ContractType::SignedToken {
        signed_token_preimage:
            groundhog_contract_spec::ContractSignedTokenPreimage {
                preimage_type: None,
            },
    }
}

#[test]
fn signed_token_args_canonicalize_and_resolve_preimage_typedefs() {
    // A signed-token arg followed by a normal calldata arg: the token must
    // stay a SignedToken node (not decay to u32), and the Named type inside
    // its preimage must pull in the typedef.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Token"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Token"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("transfer"),
                    args: vec![
                        ContractField {
                            name: s("auth"),
                            ty: signed_token(ContractType::Named {
                                named: id("TransferAuth"),
                            }),
                        },
                        ContractField {
                            name: s("amount"),
                            ty: ContractType::U64,
                        },
                    ]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: id("TransferAuth"),
                    fields: vec![ContractField {
                        name: s("amount"),
                        ty: ContractType::U64,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let method = &spec.interfaces[0].methods[0];
    assert_eq!(
        method.args[0].ty,
        signed_token(ContractType::Named {
            named: id("TransferAuth"),
        })
    );
    assert_eq!(method.args[1].ty, ContractType::U64);
    // The preimage's Named type is reachable, so its typedef is retained.
    assert_eq!(spec.type_defs.len(), 1);
}

#[test]
fn opaque_signed_token_return_type_canonicalizes() {
    // A `Vec<Signed<Opaque>>` return (a wallet handing back several tokens):
    // the erased preimage must survive canonicalization untouched.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Wallet"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Wallet"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("is_valid_signature"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Vector {
                        vector: groundhog_contract_spec::ContractVectorType {
                            element: Box::new(opaque_signed_token()),
                            max_len: u32::MAX,
                        },
                    },
                },
            },
        },
    ])
    .unwrap();

    assert_eq!(
        spec.interfaces[0].methods[0].return_type,
        ContractType::Vector {
            vector: groundhog_contract_spec::ContractVectorType {
                element: Box::new(opaque_signed_token()),
                max_len: u32::MAX,
            },
        }
    );
}

#[test]
fn signed_token_preimage_missing_typedef_is_rejected() {
    // collect_named_types must recurse into the token preimage: a Named
    // type there without a typedef is an error, not a silent omission.
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Token"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Token"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("transfer"),
                    args: vec![ContractField {
                        name: s("auth"),
                        ty: signed_token(ContractType::Named {
                            named: id("MissingAuth"),
                        }),
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::MissingTypeDef(ref name) if name == &id_str("MissingAuth")),
        "expected MissingTypeDef(MissingAuth), got {err:?}"
    );
}

// A `throws(...)` payload may not reach a signed token. The handle for a
// signed token travels in the auth-token list the host returns beside a
// *successful* return; abort frames carry no such list, so a token reached
// through an exception can never be given a real handle and would decode to
// an uninitialized sentinel.

/// Build a spec whose single `throws` entry has type `exn_ty`, alongside
/// `type_defs`.
fn canon_with_throws(
    exn_ty: ContractType,
    type_defs: Vec<ContractTypeDef>,
) -> Result<ContractSpec, SpecToolError> {
    let mut fragments = vec![
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![exn(exn_ty)].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ];
    fragments.extend(
        type_defs
            .into_iter()
            .map(|type_def| SpecFragment::SpecTypeDef { type_def }),
    );
    canon(&fragments)
}

fn struct_def(name: &str, fields: Vec<ContractField>) -> ContractTypeDef {
    ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: id(name),
            fields: fields.try_into().unwrap(),
        },
    }
}

#[test]
fn throws_signed_token_directly_is_rejected() {
    assert!(matches!(
        canon_with_throws(signed_token(ContractType::U64), vec![]),
        Err(SpecToolError::ExceptionCarriesAuthToken(_))
    ));
}

#[test]
fn throws_opaque_signed_token_is_rejected() {
    // An erased preimage is still a token: the payload is irrelevant.
    assert!(matches!(
        canon_with_throws(opaque_signed_token(), vec![]),
        Err(SpecToolError::ExceptionCarriesAuthToken(_))
    ));
}

#[test]
fn throws_struct_field_signed_token_is_rejected() {
    // The realistic shape: an error type that hands a token back.
    assert!(matches!(
        canon_with_throws(
            ContractType::Named {
                named: id("RefundError")
            },
            vec![struct_def(
                "RefundError",
                vec![ContractField {
                    name: s("returned"),
                    ty: signed_token(ContractType::U64),
                }],
            )],
        ),
        Err(SpecToolError::ExceptionCarriesAuthToken(_))
    ));
}

#[test]
fn throws_deeply_nested_signed_token_is_rejected() {
    // Wrapping in containers and another named type must not launder it.
    assert!(matches!(
        canon_with_throws(
            ContractType::Named { named: id("Outer") },
            vec![
                struct_def(
                    "Outer",
                    vec![ContractField {
                        name: s("inner"),
                        ty: ContractType::Option {
                            option_inner: Box::new(ContractType::Named {
                                named: id("Inner")
                            }),
                        },
                    }],
                ),
                struct_def(
                    "Inner",
                    vec![ContractField {
                        name: s("tokens"),
                        ty: ContractType::Vector {
                            vector: ContractVectorType {
                                element: Box::new(opaque_signed_token()),
                                max_len: u32::MAX,
                            },
                        },
                    }],
                ),
            ],
        ),
        Err(SpecToolError::ExceptionCarriesAuthToken(_))
    ));
}

#[test]
fn throws_recursive_type_without_token_is_accepted() {
    // A self-referential exception payload must terminate the walk rather
    // than recurse forever — and must not be falsely accused.
    let spec = canon_with_throws(
        ContractType::Named {
            named: id("TreeError"),
        },
        vec![struct_def(
            "TreeError",
            vec![
                ContractField {
                    name: s("code"),
                    ty: ContractType::U32,
                },
                ContractField {
                    name: s("cause"),
                    ty: ContractType::Option {
                        option_inner: Box::new(ContractType::Named {
                            named: id("TreeError"),
                        }),
                    },
                },
            ],
        )],
    );

    assert!(spec.is_ok(), "recursive payload rejected: {spec:?}");
}

#[test]
fn throws_recursive_type_reaching_token_is_still_rejected() {
    // Recursion must not become an escape hatch: the token is reachable on a
    // finite path, so the visited-set guard must not mask it.
    assert!(matches!(
        canon_with_throws(
            ContractType::Named {
                named: id("TreeError")
            },
            vec![struct_def(
                "TreeError",
                vec![
                    ContractField {
                        name: s("claim"),
                        ty: opaque_signed_token(),
                    },
                    ContractField {
                        name: s("cause"),
                        ty: ContractType::Option {
                            option_inner: Box::new(ContractType::Named {
                                named: id("TreeError")
                            }),
                        },
                    },
                ],
            )],
        ),
        Err(SpecToolError::ExceptionCarriesAuthToken(_))
    ));
}

#[test]
fn throws_without_signed_token_is_accepted() {
    let spec = canon_with_throws(
        ContractType::Named {
            named: id("PlainError"),
        },
        vec![struct_def(
            "PlainError",
            vec![ContractField {
                name: s("code"),
                ty: ContractType::U32,
            }],
        )],
    );

    assert!(spec.is_ok(), "plain payload rejected: {spec:?}");
}

#[test]
fn signed_token_in_method_argument_is_still_allowed() {
    // The rule is specific to exception payloads: passing and returning
    // signed tokens on the success path is the whole point of the feature.
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("deposit"),
                    args: vec![ContractField {
                        name: s("token"),
                        ty: signed_token(ContractType::U64),
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: opaque_signed_token(),
                },
            },
        },
    ]);

    assert!(spec.is_ok(), "signed token in method rejected: {spec:?}");
}

// ---------------------------------------------------------------------------
// Qualified type ids.
//
// A type names itself by its fully qualified id
// (`CRATE@MAJOR::module::path::Name`), which is unique by construction, and
// the finalized spec keeps that id rather than shortening it. Two crates (or
// two modules of one crate, or two major versions of one crate) may each
// define a `Meta`, and neither author can be asked to rename for the other's
// sake; carrying the id means the spec can say which is which, and a client
// reading two deployed contracts can tell whether their `Meta`s are the same
// type.
//
// Shortening to the bare name that generated bindings declare — and numbering
// the losers when two ids share a final segment — belongs to each codegen,
// which is the only layer that knows what its target language needs. Nothing
// here reduces names.
// ---------------------------------------------------------------------------

/// A fully segmented id, for the tests that turn on identity rather than on
/// a name. Unlike `id`, every segment varies, so a canonicalizer that ignored
/// the crate, major or module would be caught.
fn seg(
    krate: &str,
    major: &str,
    module: &str,
    declared: &str,
) -> ContractSpecId {
    ContractSpecId {
        crate_name: s(krate),
        major: s(major),
        module_path: s(module),
        declared: s(declared),
    }
}

/// A reference to a segmented id.
fn named_of(id: ContractSpecId) -> ContractType {
    ContractType::Named { named: id }
}

/// The one struct-def constructor: a def named by the whole id `id`, with
/// fields given as `(name, type)` pairs.
fn struct_of(
    id: ContractSpecId,
    fields: &[(&str, ContractType)],
) -> ContractTypeDef {
    ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: id,
            fields: fields
                .iter()
                .map(|(name, ty)| ContractField {
                    name: s(name),
                    ty: ty.clone(),
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        },
    }
}

/// Every type def's whole id, in output order.
fn type_def_ids(spec: &ContractSpec) -> Vec<ContractSpecId> {
    spec.type_defs
        .iter()
        .map(|type_def| match type_def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                struct_def.name.clone()
            }
            ContractTypeDef::TypeDefEnum { enum_def } => enum_def.name.clone(),
            ContractTypeDef::TypeDefUnion { union_def } => {
                union_def.name.clone()
            }
            _ => panic!("unexpected variant"),
        })
        .collect()
}

/// `(field name, whole id it references)` for the struct with this id.
fn field_ref_ids(
    spec: &ContractSpec,
    id: &ContractSpecId,
) -> Vec<(String, ContractSpecId)> {
    spec.type_defs
        .iter()
        .find_map(|type_def| match type_def {
            ContractTypeDef::TypeDefStruct { struct_def }
                if &struct_def.name == id =>
            {
                Some(
                    struct_def
                        .fields
                        .iter()
                        .map(|f| {
                            let target = match &f.ty {
                                ContractType::Named { named } => named.clone(),
                                other => {
                                    panic!("{other:?} is not a named type")
                                }
                            };
                            (spec_str(&f.name).to_string(), target)
                        })
                        .collect(),
                )
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no struct with id {id:?}"))
}

/// [`struct_of`] for the tests that turn on a name rather than on identity:
/// the def is named by `id(sym)`, so every other segment is the fixture's.
fn id_struct(sym: &str, fields: &[(&str, ContractType)]) -> ContractTypeDef {
    struct_of(id(sym), fields)
}

fn named(sym: &str) -> ContractType {
    ContractType::Named { named: id(sym) }
}

/// The spec name each type def carries, in output order.
fn type_def_names(spec: &ContractSpec) -> Vec<&str> {
    spec.type_defs
        .iter()
        .map(|type_def| match type_def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name.declared)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name.declared)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name.declared)
            }
            _ => panic!("unexpected variant"),
        })
        .collect()
}

/// The named type each field of struct `name` refers to, as `(field, target)`.
fn iface_returning(return_type: ContractType) -> Vec<SpecFragment> {
    vec![
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("get"),
                    args: vec![].try_into().unwrap(),
                    return_type,
                },
            },
        },
    ]
}

#[test]
fn a_qualified_id_survives_canonicalization_intact() {
    let meta = seg("mycrate", "1", "mycrate::mymod", "Meta");
    let mut fragments = iface_returning(named_of(meta.clone()));
    fragments.push(SpecFragment::SpecTypeDef {
        type_def: struct_of(meta.clone(), &[("count", ContractType::U32)]),
    });

    let spec = canon(&fragments).unwrap();

    // Every segment, not just the declared name: canonicalization must not
    // drop, rewrite or normalize any part of an id.
    assert_eq!(type_def_ids(&spec), vec![meta]);
}

/// The four `Meta`s below each differ from the first on exactly one identity
/// axis, so a canonicalizer that ignored any single field would collapse a
/// pair of them and fail the tests built on this fixture. They are otherwise
/// indistinguishable: one declared name, and a module path that only the
/// last one varies.
fn four_metas() -> [ContractSpecId; 4] {
    [
        seg("acrate", "1", "m", "Meta"),
        // differs from the first by crate alone
        seg("bcrate", "1", "m", "Meta"),
        // by major alone
        seg("acrate", "2", "m", "Meta"),
        // by module alone
        seg("acrate", "1", "m::inner", "Meta"),
    ]
}

/// Guards the fixture itself: one shared declared name, and exactly one
/// differing field per id. Were a pair to differ on two axes, a canonicalizer
/// that ignored one of them would still tell that pair apart, and the tests
/// built on these ids would quietly stop covering that field.
#[test]
fn each_meta_differs_from_the_first_on_exactly_one_field() {
    let [base, rest @ ..] = four_metas();
    assert_eq!(spec_str(&base.declared), "Meta", "{base:?}");
    for other in &rest {
        assert_eq!(spec_str(&other.declared), "Meta", "{other:?}");
        let differing = [
            base.crate_name != other.crate_name,
            base.major != other.major,
            base.module_path != other.module_path,
        ]
        .into_iter()
        .filter(|differs| *differs)
        .count();
        assert_eq!(
            differing, 1,
            "{other:?} must differ from {base:?} on exactly one field"
        );
    }
}

/// Four types sharing the declared name `Meta`, told apart only by crate,
/// major and module. Both halves of one run: each survives canonicalization
/// under its own id, and each reference still carries the whole id of the one
/// it means. The two turn on the same field-to-id correspondence, so they
/// read it from the same table.
#[test]
fn ids_sharing_a_final_segment_survive_and_keep_their_references() {
    let [a, b, a_v2, a_inner] = four_metas();
    let wrapper = seg("wrap", "1", "wrap", "Wrapper");
    // Distinct field shapes as well as distinct ids: collapsing any pair
    // would be caught as a conflicting shape even if the ids went unread.
    let metas = [
        ("a", a.clone(), ContractType::U32),
        ("b", b.clone(), ContractType::String),
        ("a_v2", a_v2.clone(), ContractType::U64),
        ("a_inner", a_inner.clone(), ContractType::Bool),
    ];

    let mut fragments = iface_returning(named_of(wrapper.clone()));
    fragments.extend(metas.iter().map(|(field, id, ty)| {
        SpecFragment::SpecTypeDef {
            type_def: struct_of(id.clone(), &[(*field, ty.clone())]),
        }
    }));
    fragments.push(SpecFragment::SpecTypeDef {
        type_def: struct_of(
            wrapper.clone(),
            &metas
                .iter()
                .map(|(field, id, _)| (*field, named_of(id.clone())))
                .collect::<Vec<_>>(),
        ),
    });

    let spec = canon(&fragments).unwrap();

    // All five are kept under their own ids. Nothing is renamed or merged, so
    // nothing depends on the order the fragments were linked in.
    let mut got = type_def_ids(&spec);
    let mut want = vec![a, b, a_v2, a_inner, wrapper.clone()];
    got.sort();
    want.sort();
    assert_eq!(got, want);

    // And four fields whose types share a declared name point at four
    // different spec types, each still named by its whole id.
    assert_eq!(
        field_ref_ids(&spec, &wrapper),
        metas
            .iter()
            .map(|(field, id, _)| ((*field).to_string(), id.clone()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn identical_defs_under_one_id_still_dedup() {
    // The ordinary case: one crate, linked once, emitting its type def from
    // more than one translation unit.
    let def = id_struct("mycrate@1::Meta", &[("count", ContractType::U32)]);
    let mut fragments = iface_returning(named("mycrate@1::Meta"));
    fragments.extend([
        SpecFragment::SpecTypeDef {
            type_def: def.clone(),
        },
        SpecFragment::SpecTypeDef { type_def: def },
    ]);

    let spec = canon(&fragments).unwrap();

    assert_eq!(type_def_names(&spec), vec!["mycrate@1::Meta"]);
}

#[test]
fn different_shapes_under_one_id_are_still_rejected() {
    // Same crate, same major version, two different shapes: either a semver
    // violation (we are stricter than semver here — a struct's fields are its
    // wire format, so adding one is breaking) or a hand-written id claiming a
    // type it does not own. Either way it is ambiguous and must not resolve
    // silently.
    let mut fragments = iface_returning(named("mycrate@1::Meta"));
    fragments.extend([
        SpecFragment::SpecTypeDef {
            type_def: id_struct(
                "mycrate@1::Meta",
                &[("count", ContractType::U32)],
            ),
        },
        SpecFragment::SpecTypeDef {
            type_def: id_struct(
                "mycrate@1::Meta",
                &[("count", ContractType::U64)],
            ),
        },
    ]);

    let err = canon(&fragments).unwrap_err();

    // The id, not the shortened name: the id is what disagrees.
    assert_eq!(
        err,
        SpecToolError::ConflictingTypeDef(id_str("mycrate@1::Meta"))
    );
}

#[test]
fn an_event_and_a_type_sharing_a_final_segment_both_survive() {
    // Bindings declare a type per event, so an event and a type sharing a
    // final segment collide in a target language's namespace — but that is
    // the codegen's collision to resolve, and both keep their own id here.
    let mut fragments = vec![
        iface_with_emits("Named", &["evt@1::Transfer"]),
        anchor("Named"),
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                declared_by: declarer("test_if", "Iface"),
                method: ContractSpecMethod {
                    name: s("get"),
                    args: vec![].try_into().unwrap(),
                    return_type: named("ty@1::Transfer"),
                },
            },
        },
    ];
    fragments.extend([
        SpecFragment::SpecTypeDef {
            type_def: id_struct(
                "ty@1::Transfer",
                &[("count", ContractType::U32)],
            ),
        },
        SpecFragment::SpecEvent {
            event_def: event("evt@1::Transfer"),
        },
    ]);

    let spec = canonicalize_spec(&fragments).unwrap();

    let event_names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(type_def_names(&spec), vec!["ty@1::Transfer"]);
    assert_eq!(event_names, vec!["evt@1::Transfer"]);
}

/// An event's signature is topic 0 of every emission, and is hashed from
/// (crate, MAJOR, ident) — less than the spec name distinguishes. Two events
/// in different modules of one crate therefore get distinct ids and one
/// signature, and the spec would be promising a distinction the wire cannot
/// deliver. Canonicalization rejects it.
#[test]
fn retained_events_sharing_a_signature_are_rejected() {
    let collide = |name: &str| ContractEventDef {
        name: id(name),
        signature: signature_for("Updated").into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    };

    let err = canonicalize_spec(&[
        iface_with_emits(
            "Named",
            &["evt@1::ledger::Updated", "evt@1::vault::Updated"],
        ),
        anchor("Named"),
        SpecFragment::SpecEvent {
            event_def: collide("evt@1::ledger::Updated"),
        },
        SpecFragment::SpecEvent {
            event_def: collide("evt@1::vault::Updated"),
        },
    ])
    .unwrap_err();

    let SpecToolError::ConflictingEventSignature(msg) = err else {
        panic!("expected a signature conflict, got {err:?}");
    };
    assert!(msg.contains("evt@1::ledger::Updated"), "{msg}");
    assert!(msg.contains("evt@1::vault::Updated"), "{msg}");
}

/// The check is over retained events only, like every other conflict here:
/// two shaken candidates that collide are nobody's problem.
#[test]
fn shaken_events_sharing_a_signature_are_ignored() {
    let collide = |name: &str| ContractEventDef {
        name: id(name),
        signature: signature_for("Updated").into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    };

    let spec = canonicalize_spec(&[
        iface_with_emits("Named", &["evt@1::ledger::Updated"]),
        anchor("Named"),
        SpecFragment::SpecEvent {
            event_def: collide("evt@1::ledger::Updated"),
        },
        // Never emitted by an anchored interface, so it is shaken before the
        // signature check sees it.
        SpecFragment::SpecEvent {
            event_def: collide("evt@1::vault::Updated"),
        },
    ])
    .unwrap();

    let names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, vec!["evt@1::ledger::Updated"]);
}

/// Distinct signatures are the ordinary case and must keep working: two
/// events from different crates share an ident but not a signature, because
/// the crate name is in the preimage.
#[test]
fn retained_events_with_distinct_signatures_are_kept() {
    let spec = canonicalize_spec(&[
        iface_with_emits("Named", &["a@1::Updated", "b@1::Updated"]),
        anchor("Named"),
        SpecFragment::SpecEvent {
            event_def: event("a@1::Updated"),
        },
        SpecFragment::SpecEvent {
            event_def: event("b@1::Updated"),
        },
    ])
    .unwrap();

    let names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, vec!["a@1::Updated", "b@1::Updated"]);
}

/// An `emits` entry is four unvalidated XDR strings, not one.
///
/// Only `declared` is read by the emptiness check beside it, so validating
/// just that segment would let a malformed crate, major or module path
/// through: the reference would reach event lookup and be reported as a
/// missing event, blaming the wrong thing. Every segment is checked.
#[test]
fn an_emits_entry_with_a_non_utf8_segment_is_rejected_as_a_bad_fragment() {
    for (label, bad) in [("crate_name", 0), ("major", 1), ("module_path", 2)] {
        let mut wrong = id("Ev");
        let invalid =
            ContractSpecString::try_from(vec![0xC0u8, 0x80]).unwrap();
        match bad {
            0 => wrong.crate_name = invalid,
            1 => wrong.major = invalid,
            _ => wrong.module_path = invalid,
        }
        let err = canon(&[
            SpecFragment::SpecInterface {
                interface_fragment: SpecInterfaceFragment {
                    interface_name: s("Named"),
                    interface_version: s("0.1.0"),
                    declared_by: declarer("testcrate", "Named"),
                    throws: vec![].try_into().unwrap(),
                    emits: vec![wrong].try_into().unwrap(),
                },
            },
            anchor("Named"),
        ])
        .unwrap_err();
        assert!(
            matches!(err, SpecToolError::InvalidRawFragment(_)),
            "a bad {label} segment must be rejected as an invalid fragment, \
             got {err:?}",
        );
    }
}
// --- Interface declarers -------------------------------------------------
//
// An interface key is (name, version): all the wire carries, and all two
// unrelated traits need to collide on. Every interface-keyed fragment also
// names its declarer, so shaking keeps the anchored declarer's fragments only,
// and an artifact anchoring two declarers under one key is refused.

fn declarer_a() -> ContractSpecId {
    declarer("iface_a", "Token")
}

fn declarer_b() -> ContractSpecId {
    declarer("iface_b", "Token")
}

fn declared_iface(
    declared_by: ContractSpecId,
    emits: &[&str],
) -> SpecFragment {
    SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Token"),
            interface_version: s("0.1.0"),
            declared_by: declared_by.clone(),
            throws: vec![].try_into().unwrap(),
            emits: emits
                .iter()
                .map(|e| id(e))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        },
    }
}

fn declared_method(declared_by: ContractSpecId, name: &str) -> SpecFragment {
    SpecFragment::SpecMethod {
        method_fragment: SpecMethodFragment {
            interface_name: s("Token"),
            interface_version: s("0.1.0"),
            declared_by: declared_by.clone(),
            method: ContractSpecMethod {
                name: s(name),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::U32,
            },
        },
    }
}

fn declared_anchor(declared_by: ContractSpecId) -> SpecFragment {
    SpecFragment::InterfaceAnchor {
        interface_anchor: SpecInterfaceAnchor {
            interface_name: s("Token"),
            interface_version: s("0.1.0"),
            declared_by: declared_by.clone(),
        },
    }
}

#[test]
fn two_declarers_anchored_under_one_key_are_rejected() {
    // A contract implementing two unrelated `Token` traits: distinct methods,
    // so the entrypoints link, but one interface id for both.
    let err = canonicalize_spec(&[
        declared_iface(declarer_a(), &[]),
        declared_method(declarer_a(), "ping"),
        declared_anchor(declarer_a()),
        declared_iface(declarer_b(), &[]),
        declared_method(declarer_b(), "pong"),
        declared_anchor(declarer_b()),
    ])
    .unwrap_err();
    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceDeclarers(_)),
        "{err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains(&declarer_str("iface_a", "Token"))
            && msg.contains(&declarer_str("iface_b", "Token")),
        "{msg}"
    );
}

#[test]
fn an_unanchored_colliders_metadata_does_not_conflict_with_the_anchored_one() {
    // The contract implements A's `Token` and merely links B's (to call a B
    // token through its client). Before declarers, B's candidate landed in the
    // anchored key's bucket and its different `emits` failed the build.
    let spec = canonicalize_spec(&[
        declared_iface(declarer_a(), &["PingEvent"]),
        SpecFragment::SpecEvent {
            event_def: event("PingEvent"),
        },
        declared_method(declarer_a(), "ping"),
        declared_anchor(declarer_a()),
        declared_iface(declarer_b(), &[]),
    ])
    .unwrap();
    assert_eq!(spec.interfaces.len(), 1);
    let names: Vec<&str> = spec
        .events
        .iter()
        .map(|e| spec_str(&e.name.declared))
        .collect();
    assert_eq!(names, ["PingEvent"], "A's emits must be the ones kept");
}

#[test]
fn an_unanchored_colliders_methods_are_not_merged_in() {
    // Method fragments normally come only from an anchoring site, but a
    // carrier or test harness can link one for a collider. Its methods belong
    // to the collider, not to the anchored interface.
    let spec = canonicalize_spec(&[
        declared_iface(declarer_a(), &[]),
        declared_method(declarer_a(), "ping"),
        declared_anchor(declarer_a()),
        declared_iface(declarer_b(), &[]),
        declared_method(declarer_b(), "pong"),
    ])
    .unwrap();
    let methods: Vec<&str> = spec.interfaces[0]
        .methods
        .iter()
        .map(|m| spec_str(&m.name))
        .collect();
    assert_eq!(methods, ["ping"]);
}

#[test]
fn only_the_anchored_declarers_candidate_counts_as_the_interface_fragment() {
    // An anchor whose own declarer contributed no interface fragment is
    // missing its fragment, even if a collider's candidate sits on the key.
    let err = canonicalize_spec(&[
        declared_method(declarer_a(), "ping"),
        declared_anchor(declarer_a()),
        declared_iface(declarer_b(), &[]),
    ])
    .unwrap_err();
    assert!(
        matches!(err, SpecToolError::MissingInterfaceFragment(_)),
        "{err}"
    );
}

#[test]
fn an_interface_fragment_without_a_declarer_is_rejected() {
    for fragment in [
        declared_iface(no_declarer(), &[]),
        declared_method(no_declarer(), "ping"),
        declared_anchor(no_declarer()),
    ] {
        let err = canonicalize_spec(&[fragment]).unwrap_err();
        assert!(matches!(err, SpecToolError::InvalidRawFragment(_)), "{err}");
    }
}

#[test]
fn the_declarer_is_build_time_only_and_never_reaches_the_final_spec() {
    // `declared_by` exists to shake and check raw fragments. The published
    // `ContractSpec` has no field for it, and must not change with it: the
    // same interface declared under two different ids finalizes to the same
    // bytes.
    let finalize = |declared_by: ContractSpecId| {
        let spec = canonicalize_spec(&[
            declared_iface(declared_by.clone(), &[]),
            declared_method(declared_by.clone(), "ping"),
            declared_anchor(declared_by),
        ])
        .unwrap();
        encode(&spec).unwrap()
    };
    let a = finalize(declarer_a());
    assert_eq!(a, finalize(declarer_b()));
    for declarer in [declarer_a(), declarer_b()] {
        // The crate segment is what distinguishes these two declarers, so it
        // is what must not survive into the finalized spec.
        let krate: &[u8] = &declarer.crate_name;
        assert!(
            !a.windows(krate.len()).any(|w| w == krate),
            "{} leaked into the final spec",
            spec_str(&declarer.crate_name),
        );
    }
}

fn versioned_anchor(
    declared_by: ContractSpecId,
    version: &str,
) -> Vec<SpecFragment> {
    let key = |f: SpecFragment| match f {
        SpecFragment::SpecInterface {
            mut interface_fragment,
        } => {
            interface_fragment.interface_version = s(version);
            SpecFragment::SpecInterface { interface_fragment }
        }
        SpecFragment::InterfaceAnchor {
            mut interface_anchor,
        } => {
            interface_anchor.interface_version = s(version);
            SpecFragment::InterfaceAnchor { interface_anchor }
        }
        other => other,
    };
    vec![
        key(declared_iface(declared_by.clone(), &[])),
        key(declared_anchor(declared_by)),
    ]
}

#[test]
fn declarers_collide_across_full_versions_that_share_a_major() {
    // `1.0.0` and `1.1.0` are separate keys, but the wire hashes only the
    // major, so the two interfaces share an interface id all the same.
    let mut fragments = versioned_anchor(declarer_a(), "1.0.0");
    fragments.extend(versioned_anchor(declarer_b(), "1.1.0"));
    let err = canonicalize_spec(&fragments).unwrap_err();
    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceDeclarers(_)),
        "{err}"
    );
    assert!(err.to_string().contains("Token@1 "), "{err}");
}

#[test]
fn declarers_at_different_majors_do_not_collide() {
    // Distinct majors are distinct wire identities, so two declarers may
    // each own one.
    let mut fragments = versioned_anchor(declarer_a(), "1.0.0");
    fragments.extend(versioned_anchor(declarer_b(), "2.0.0"));
    let spec = canonicalize_spec(&fragments).unwrap();
    assert_eq!(spec.interfaces.len(), 2);
}
