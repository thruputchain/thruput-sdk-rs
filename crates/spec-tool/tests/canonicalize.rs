use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractExceptionType,
    ContractField, ContractSpec, ContractSpecMethod, ContractSpecString,
    ContractStructDef, ContractType, ContractTypeDef, ContractUnionCase,
    ContractUnionCaseType, ContractUnionDef, SpecFragment,
    SpecInterfaceAnchor, SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec, parse_raw_fragments,
};
use xdrs::txs::encode;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
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
                ));
            }
            SpecFragment::SpecMethod { method_fragment: m } => {
                keys.insert((
                    m.interface_name.clone(),
                    m.interface_version.clone(),
                ));
            }
            _ => {}
        }
    }
    for (interface_name, interface_version) in keys {
        v.push(SpecFragment::InterfaceAnchor {
            interface_anchor: SpecInterfaceAnchor {
                interface_name,
                interface_version,
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
    let spec = canonicalize_spec(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }])
    .unwrap();
    assert!(
        spec.interfaces.is_empty(),
        "an unanchored interface must be shaken out, got {:?}",
        spec.interfaces
    );
}

#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canon(&[
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![ContractField {
                        name: s("request"),
                        ty: ContractType::Named {
                            named: s("Payload"),
                        },
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Named {
                        named: s("Response"),
                    },
                },
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                    name: s("DemoError"),
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
                    name: s("Payload"),
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
                    name: s("Response"),
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
                    spec_str(&struct_def.name),
                ContractTypeDef::TypeDefEnum { enum_def } =>
                    spec_str(&enum_def.name),
                ContractTypeDef::TypeDefUnion { union_def } =>
                    spec_str(&union_def.name),
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
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                    name: s("DemoError"),
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: method.clone(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
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
            name: s("Payload"),
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
            name: s("Response"),
            fields: vec![ContractField {
                name: s("payload"),
                ty: ContractType::Named {
                    named: s("Payload"),
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: s("Response"),
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
                    spec_str(&struct_def.name),
                ContractTypeDef::TypeDefEnum { enum_def } =>
                    spec_str(&enum_def.name),
                ContractTypeDef::TypeDefUnion { union_def } =>
                    spec_str(&union_def.name),
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
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
        matches!(err, SpecToolError::MissingTypeDef(name) if name == "DemoError")
    );
}

#[test]
fn canonicalize_spec_rejects_method_only_interface_buckets() {
    let err = canon(&[SpecFragment::SpecMethod {
        method_fragment: SpecMethodFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
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
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                    name: s("DemoError"),
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
                    name: s("DemoError"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingTypeDef(name) if name == "DemoError")
    );
}

#[test]
fn canonicalize_spec_rejects_unreachable_conflicting_type_defs() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
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
                    name: s("NeverUsed"),
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
                    name: s("NeverUsed"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingTypeDef(name) if name == "NeverUsed")
    );
}

#[test]
fn canonicalize_spec_rejects_conflicting_interface_throws() {
    let err = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                throws: vec![exn(ContractType::Named {
                    named: s("OtherError"),
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
                    name: s("DemoError"),
                    variants: vec![].try_into().unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: s("OtherError"),
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
                throws: vec![exn(ContractType::Named {
                    named: s("DemoError"),
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
                    name: s("DemoError"),
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
            throws: vec![].try_into().unwrap(),
            emits: vec![].try_into().unwrap(),
        },
    }])
    .unwrap_err();

    assert_eq!(
        err,
        SpecToolError::InvalidRawFragment(
            "interface version must be MAJOR.MINOR.PATCH, got v1".into()
        )
    );
}

#[test]
fn canonicalize_spec_resolves_types_in_multi_field_union_cases() {
    let spec = canon(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("get"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: s("Envelope"),
                    },
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefUnion {
                union_def: ContractUnionDef {
                    name: s("Envelope"),
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
                                                named: s("Payload"),
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
                                        named: s("Payload"),
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
                    name: s("Payload"),
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
                spec_str(&struct_def.name)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name)
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
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
                    name: s("ExportedEvent"),
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
                    name: s("NeverUsedHelper"),
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
                spec_str(&struct_def.name)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name)
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
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
                    name: s("ExportedEvent"),
                    fields: vec![ContractField {
                        name: s("meta"),
                        ty: ContractType::Named {
                            named: s("Metadata"),
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
                    name: s("Metadata"),
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
                spec_str(&struct_def.name)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name)
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
    ]
}

fn event_def(name: &str, payload_ty: ContractType) -> ContractEventDef {
    ContractEventDef {
        name: s(name),
        signature: [0xAB; 32].into(),
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

    let names: Vec<_> =
        spec.events.iter().map(|e| spec_str(&e.name)).collect();
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
                named: s("Payload"),
            },
        ),
    });
    fragments.push(SpecFragment::SpecTypeDef {
        type_def: ContractTypeDef::TypeDefStruct {
            struct_def: ContractStructDef {
                name: s("Payload"),
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
                named: s("Payload"),
            },
        ),
    });
    assert_eq!(
        canonicalize_spec(&missing),
        Err(SpecToolError::MissingTypeDef("Payload".to_owned()))
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
        Err(SpecToolError::ConflictingEventDef(
            "TransferEvent".to_owned()
        ))
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
            name: s("NoisyEvent"),
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
            name: s(""),
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
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Demo"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Named {
                        named: ContractSpecString::try_from(vec![
                            0xC0u8, 0x80,
                        ])
                        .unwrap(),
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
                throws: vec![
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::Named { named: s("ErrA") },
                    },
                    ContractExceptionType {
                        selector: 7,
                        ty: ContractType::Named { named: s("ErrB") },
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
        name: s(name),
        signature: [0u8; 32].into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    }
}

fn iface_with_emits(name: &str, emits: &[&str]) -> SpecFragment {
    SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            emits: emits
                .iter()
                .map(|e| s(e))
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
    let names: Vec<&str> =
        spec.events.iter().map(|e| spec_str(&e.name)).collect();
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
    let names: Vec<&str> =
        spec.events.iter().map(|e| spec_str(&e.name)).collect();
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
