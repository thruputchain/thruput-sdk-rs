use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpecMethod,
    ContractStructDef, ContractType, ContractTypeDef, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec, parse_raw_fragments,
};
use xdrs::txs::encode;

#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canonicalize_spec(&[
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![ContractField {
                    name: "request".into(),
                    ty: ContractType::Named("Payload".into()),
                }],
                return_type: ContractType::Named("Response".into()),
            },
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "pong".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".into(),
            variants: vec![ContractEnumVariant {
                name: "Nope".into(),
                value: 7,
            }],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
            name: "Payload".into(),
            fields: vec![ContractField {
                name: "count".into(),
                ty: ContractType::U32,
            }],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
            name: "Response".into(),
            fields: vec![ContractField {
                name: "ok".into(),
                ty: ContractType::Bool,
            }],
        })),
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec.interfaces[0].name, "Named");
    assert_eq!(spec.interfaces[0].version, "0.1.0");
    assert_eq!(
        spec.interfaces[0]
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        vec!["ping", "pong"]
    );
    assert_eq!(
        spec.type_defs
            .iter()
            .map(|type_def| match type_def {
                ContractTypeDef::Struct(def) => def.name.as_str(),
                ContractTypeDef::Enum(def) => def.name.as_str(),
                ContractTypeDef::Union(def) => def.name.as_str(),
            })
            .collect::<Vec<_>>(),
        vec!["DemoError", "Payload", "Response"]
    );
}

#[test]
fn canonicalize_spec_merges_repeated_interface_buckets_when_throws_agree() {
    let spec = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "zeta".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "alpha".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".into(),
            variants: vec![],
        })),
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(
        spec.interfaces[0]
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}

#[test]
fn canonicalize_spec_dedups_identical_methods() {
    let method = ContractSpecMethod {
        name: "alpha".into(),
        args: vec![ContractField {
            name: "value".into(),
            ty: ContractType::U32,
        }],
        return_type: ContractType::Void,
    };

    let spec = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: method.clone(),
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method,
        }),
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec.interfaces[0].methods.len(), 1);
    assert_eq!(spec.interfaces[0].methods[0].name, "alpha");
}

#[test]
fn canonicalize_spec_dedups_identical_type_defs() {
    let payload = ContractTypeDef::Struct(ContractStructDef {
        name: "Payload".into(),
        fields: vec![ContractField {
            name: "count".into(),
            ty: ContractType::U32,
        }],
    });
    let response = ContractTypeDef::Struct(ContractStructDef {
        name: "Response".into(),
        fields: vec![ContractField {
            name: "payload".into(),
            ty: ContractType::Named("Payload".into()),
        }],
    });

    let spec = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Named("Response".into()),
            },
        }),
        SpecFragment::TypeDef(payload.clone()),
        SpecFragment::TypeDef(response),
        SpecFragment::TypeDef(payload),
    ])
    .unwrap();

    assert_eq!(
        spec.type_defs
            .iter()
            .map(|type_def| match type_def {
                ContractTypeDef::Struct(def) => def.name.as_str(),
                ContractTypeDef::Enum(def) => def.name.as_str(),
                ContractTypeDef::Union(def) => def.name.as_str(),
            })
            .collect::<Vec<_>>(),
        vec!["Payload", "Response"]
    );
}

#[test]
fn canonicalize_spec_rejects_missing_named_type_defs() {
    let err = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::MissingTypeDef(name) if name == "DemoError")
    );
}

#[test]
fn canonicalize_spec_rejects_method_only_interface_buckets() {
    let err = canonicalize_spec(&[SpecFragment::Method(SpecMethodFragment {
        interface_name: "Named".into(),
        interface_version: "0.1.0".into(),
        method: ContractSpecMethod {
            name: "ping".into(),
            args: vec![],
            return_type: ContractType::Void,
        },
    })])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::MissingInterfaceFragment(key) if key == "Named@0.1.0")
    );
}

#[test]
fn canonicalize_spec_rejects_conflicting_type_defs() {
    let err = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".into(),
            variants: vec![ContractEnumVariant {
                name: "Nope".into(),
                value: 7,
            }],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
            name: "DemoError".into(),
            fields: vec![],
        })),
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingTypeDef(name) if name == "DemoError")
    );
}

#[test]
fn canonicalize_spec_rejects_unreachable_conflicting_type_defs() {
    let err = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "NeverUsed".into(),
            variants: vec![ContractEnumVariant {
                name: "Left".into(),
                value: 1,
            }],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
            name: "NeverUsed".into(),
            fields: vec![],
        })),
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingTypeDef(name) if name == "NeverUsed")
    );
}

#[test]
fn canonicalize_spec_rejects_conflicting_interface_throws() {
    let err = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("OtherError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".into(),
            variants: vec![],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "OtherError".into(),
            variants: vec![],
        })),
    ])
    .unwrap_err();

    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceThrows(key) if key == "Named@0.1.0")
    );
}

#[test]
fn parse_raw_fragments_decodes_concatenated_xdr_stream() {
    let fragments = vec![
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("DemoError".into())],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![ContractField {
                    name: "value".into(),
                    ty: ContractType::U32,
                }],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".into(),
            variants: vec![ContractEnumVariant {
                name: "Nope".into(),
                value: 7,
            }],
        })),
    ];

    let mut bytes = Vec::new();
    for fragment in &fragments {
        bytes.extend(encode(fragment).unwrap());
    }

    assert_eq!(parse_raw_fragments(&bytes).unwrap(), fragments);
}

#[test]
fn canonicalize_spec_rejects_non_semver_interface_versions() {
    let err =
        canonicalize_spec(&[SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "v1".into(),
            throws: vec![],
        })])
        .unwrap_err();

    assert_eq!(
        err,
        SpecToolError::InvalidRawFragment(
            "interface version must be MAJOR.MINOR.PATCH, got v1".into()
        )
    );
}
