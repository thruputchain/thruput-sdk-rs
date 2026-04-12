use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractUnionCase, ContractUnionCaseType, ContractUnionDef, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec, parse_raw_fragments,
};
use xdrs::txs::encode;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn spec_str(value: &ContractSpecString) -> &str {
    std::str::from_utf8(value).unwrap()
}

#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canonicalize_spec(&[
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
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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
    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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

    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
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
    let err = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
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

    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
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
    let err = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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
    let err = canonicalize_spec(&[SpecFragment::SpecMethod {
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
    let err = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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
    let err = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
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
    let err = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
            },
        },
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![ContractType::Named {
                    named: s("OtherError"),
                }]
                .try_into()
                .unwrap(),
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
                throws: vec![ContractType::Named {
                    named: s("DemoError"),
                }]
                .try_into()
                .unwrap(),
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
    let err = canonicalize_spec(&[SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("v1"),
            throws: vec![].try_into().unwrap(),
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
    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
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
