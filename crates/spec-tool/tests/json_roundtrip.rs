use groundhog_contract_spec::{
    ContractArrayType, ContractEnumDef, ContractEnumVariant,
    ContractExceptionType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDef, ContractUnionCase, ContractUnionCaseType,
    ContractUnionDef, ContractVectorType,
};
use groundhog_spec_tool::{parse_spec_json, spec_to_json};

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn exn(ty: ContractType) -> ContractExceptionType {
    ContractExceptionType { selector: 0, ty }
}

fn roundtrip(spec: &ContractSpec) {
    let json = spec_to_json(spec).expect("spec_to_json must succeed");
    let parsed = parse_spec_json(&json).unwrap_or_else(|err| {
        panic!("failed to parse JSON back:\n{json}\nerror: {err}")
    });
    assert_eq!(&parsed, spec, "roundtrip mismatch for JSON:\n{json}");
}

// ---------------------------------------------------------------------------
// Minimal / degenerate specs
// ---------------------------------------------------------------------------

#[test]
fn empty_interface_roundtrips() {
    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Empty"),
            vers: s("0.0.1"),
            throws: vec![].try_into().unwrap(),
            methods: vec![].try_into().unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![].try_into().unwrap(),
    });
}

#[test]
fn multiple_interfaces_roundtrip() {
    roundtrip(&ContractSpec {
        interfaces: vec![
            ContractSpecInterface {
                name: s("Alpha"),
                vers: s("1.0.0"),
                throws: vec![].try_into().unwrap(),
                methods: vec![].try_into().unwrap(),
            },
            ContractSpecInterface {
                name: s("Beta"),
                vers: s("2.3.4"),
                throws: vec![].try_into().unwrap(),
                methods: vec![].try_into().unwrap(),
            },
        ]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![].try_into().unwrap(),
    });
}

// ---------------------------------------------------------------------------
// Every primitive type in method signatures
// ---------------------------------------------------------------------------

#[test]
fn all_primitive_types_roundtrip() {
    let all_primitives = vec![
        ContractType::Void,
        ContractType::Bool,
        ContractType::U8,
        ContractType::U32,
        ContractType::I32,
        ContractType::U64,
        ContractType::I64,
        ContractType::U128,
        ContractType::I128,
        ContractType::String,
        ContractType::Bytes,
        ContractType::Address,
    ];
    let methods: Vec<ContractSpecMethod> = all_primitives
        .iter()
        .enumerate()
        .map(|(i, ty)| ContractSpecMethod {
            name: s(&format!("m{i}")),
            args: vec![ContractField {
                name: s("arg"),
                ty: ty.clone(),
            }]
            .try_into()
            .unwrap(),
            return_type: ty.clone(),
        })
        .collect();

    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Primitives"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: methods.try_into().unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![].try_into().unwrap(),
    });
}

// ---------------------------------------------------------------------------
// Composite types: Option, Vector, Array, Tuple, Named
// ---------------------------------------------------------------------------

#[test]
fn option_type_roundtrips() {
    roundtrip(&spec_with_method(
        "opt",
        ContractType::Option {
            option_inner: Box::new(ContractType::U64),
        },
    ));
}

#[test]
fn nested_option_roundtrips() {
    roundtrip(&spec_with_method(
        "nested_opt",
        ContractType::Option {
            option_inner: Box::new(ContractType::Option {
                option_inner: Box::new(ContractType::Bool),
            }),
        },
    ));
}

#[test]
fn vector_type_roundtrips() {
    roundtrip(&spec_with_method(
        "vec_u8",
        ContractType::Vector {
            vector: ContractVectorType {
                element: Box::new(ContractType::U8),
                max_len: u32::MAX,
            },
        },
    ));
}

#[test]
fn vector_with_bounded_len_roundtrips() {
    roundtrip(&spec_with_method(
        "bounded_vec",
        ContractType::Vector {
            vector: ContractVectorType {
                element: Box::new(ContractType::U32),
                max_len: 100,
            },
        },
    ));
}

#[test]
fn array_type_roundtrips() {
    roundtrip(&spec_with_method(
        "hash",
        ContractType::Array {
            array: ContractArrayType {
                element: Box::new(ContractType::U8),
                len: 32,
            },
        },
    ));
}

#[test]
fn tuple_type_roundtrips() {
    roundtrip(&spec_with_method(
        "pair",
        ContractType::Tuple {
            tuple: vec![ContractType::U32, ContractType::Bool]
                .try_into()
                .unwrap(),
        },
    ));
}

#[test]
fn named_type_roundtrips() {
    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Iface"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("get"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::Named {
                    named: s("Payload"),
                },
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![ContractTypeDef::TypeDefStruct {
            struct_def: ContractStructDef {
                name: s("Payload"),
                fields: vec![ContractField {
                    name: s("count"),
                    ty: ContractType::U32,
                }]
                .try_into()
                .unwrap(),
            },
        }]
        .try_into()
        .unwrap(),
    };
    roundtrip(&spec);
}

// ---------------------------------------------------------------------------
// Deeply nested composite types
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_types_roundtrip() {
    // Option<Vec<(Named, [u8; 32])>>
    let inner_tuple = ContractType::Tuple {
        tuple: vec![
            ContractType::Named { named: s("Widget") },
            ContractType::Array {
                array: ContractArrayType {
                    element: Box::new(ContractType::U8),
                    len: 32,
                },
            },
        ]
        .try_into()
        .unwrap(),
    };
    let vec_of_tuples = ContractType::Vector {
        vector: ContractVectorType {
            element: Box::new(inner_tuple),
            max_len: 256,
        },
    };
    let opt_vec = ContractType::Option {
        option_inner: Box::new(vec_of_tuples),
    };

    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Deep"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("nested"),
                args: vec![ContractField {
                    name: s("data"),
                    ty: opt_vec,
                }]
                .try_into()
                .unwrap(),
                return_type: ContractType::Void,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![ContractTypeDef::TypeDefStruct {
            struct_def: ContractStructDef {
                name: s("Widget"),
                fields: vec![ContractField {
                    name: s("id"),
                    ty: ContractType::U64,
                }]
                .try_into()
                .unwrap(),
            },
        }]
        .try_into()
        .unwrap(),
    });
}

// ---------------------------------------------------------------------------
// Type definitions: struct, enum, union
// ---------------------------------------------------------------------------

#[test]
fn struct_typedef_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: s("Point"),
            fields: vec![
                ContractField {
                    name: s("x"),
                    ty: ContractType::I32,
                },
                ContractField {
                    name: s("y"),
                    ty: ContractType::I32,
                },
            ]
            .try_into()
            .unwrap(),
        },
    }));
}

#[test]
fn empty_struct_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name: s("Unit"),
            fields: vec![].try_into().unwrap(),
        },
    }));
}

#[test]
fn enum_typedef_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefEnum {
        enum_def: ContractEnumDef {
            name: s("Color"),
            variants: vec![
                ContractEnumVariant {
                    name: s("Red"),
                    value: 0,
                },
                ContractEnumVariant {
                    name: s("Green"),
                    value: 1,
                },
                ContractEnumVariant {
                    name: s("Blue"),
                    value: 2,
                },
            ]
            .try_into()
            .unwrap(),
        },
    }));
}

#[test]
fn enum_with_nonsequential_discriminants_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefEnum {
        enum_def: ContractEnumDef {
            name: s("Sparse"),
            variants: vec![
                ContractEnumVariant {
                    name: s("A"),
                    value: 10,
                },
                ContractEnumVariant {
                    name: s("B"),
                    value: 42,
                },
                ContractEnumVariant {
                    name: s("C"),
                    value: -1,
                },
            ]
            .try_into()
            .unwrap(),
        },
    }));
}

#[test]
fn union_with_void_case_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefUnion {
        union_def: ContractUnionDef {
            name: s("MaybeData"),
            cases: vec![
                ContractUnionCase {
                    name: s("None"),
                    discriminant: 0,
                    case_type: ContractUnionCaseType::UnionCaseVoid,
                },
                ContractUnionCase {
                    name: s("Some"),
                    discriminant: 1,
                    case_type: ContractUnionCaseType::UnionCaseTuple {
                        tuple: vec![ContractType::U64].try_into().unwrap(),
                    },
                },
            ]
            .try_into()
            .unwrap(),
        },
    }));
}

#[test]
fn union_with_struct_case_roundtrips() {
    roundtrip(&spec_with_typedef(ContractTypeDef::TypeDefUnion {
        union_def: ContractUnionDef {
            name: s("Envelope"),
            cases: vec![
                ContractUnionCase {
                    name: s("Empty"),
                    discriminant: 0,
                    case_type: ContractUnionCaseType::UnionCaseVoid,
                },
                ContractUnionCase {
                    name: s("Wrapped"),
                    discriminant: 1,
                    case_type: ContractUnionCaseType::UnionCaseStruct {
                        struct_fields: vec![
                            ContractField {
                                name: s("tag"),
                                ty: ContractType::U32,
                            },
                            ContractField {
                                name: s("data"),
                                ty: ContractType::Bytes,
                            },
                        ]
                        .try_into()
                        .unwrap(),
                    },
                },
                ContractUnionCase {
                    name: s("Packed"),
                    discriminant: 2,
                    case_type: ContractUnionCaseType::UnionCaseTuple {
                        tuple: vec![ContractType::U64, ContractType::U128]
                            .try_into()
                            .unwrap(),
                    },
                },
            ]
            .try_into()
            .unwrap(),
        },
    }));
}

// ---------------------------------------------------------------------------
// Throws
// ---------------------------------------------------------------------------

#[test]
fn interface_with_throws_roundtrips() {
    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Fallible"),
            vers: s("0.1.0"),
            throws: vec![
                exn(ContractType::Named {
                    named: s("AppError"),
                }),
                exn(ContractType::Named {
                    named: s("AuthError"),
                }),
            ]
            .try_into()
            .unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("risky"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::Bool,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![
            ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: s("AppError"),
                    variants: vec![ContractEnumVariant {
                        name: s("NotFound"),
                        value: 0,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
            ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: s("AuthError"),
                    variants: vec![ContractEnumVariant {
                        name: s("Denied"),
                        value: 0,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        ]
        .try_into()
        .unwrap(),
    });
}

// ---------------------------------------------------------------------------
// Multiple args
// ---------------------------------------------------------------------------

#[test]
fn method_with_multiple_args_roundtrips() {
    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Multi"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("transfer"),
                args: vec![
                    ContractField {
                        name: s("from"),
                        ty: ContractType::Address,
                    },
                    ContractField {
                        name: s("to"),
                        ty: ContractType::Address,
                    },
                    ContractField {
                        name: s("amount"),
                        ty: ContractType::U128,
                    },
                ]
                .try_into()
                .unwrap(),
                return_type: ContractType::Bool,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![].try_into().unwrap(),
    });
}

// ---------------------------------------------------------------------------
// Kitchen-sink: exercises everything in a single spec
// ---------------------------------------------------------------------------

#[test]
fn kitchen_sink_roundtrips() {
    let spec = ContractSpec {
        interfaces: vec![
            ContractSpecInterface {
                name: s("TokenInterface"),
                vers: s("1.0.0"),
                throws: vec![exn(ContractType::Named {
                    named: s("TokenError"),
                })]
                .try_into()
                .unwrap(),
                methods: vec![
                    ContractSpecMethod {
                        name: s("balance"),
                        args: vec![ContractField {
                            name: s("owner"),
                            ty: ContractType::Address,
                        }]
                        .try_into()
                        .unwrap(),
                        return_type: ContractType::U128,
                    },
                    ContractSpecMethod {
                        name: s("transfer"),
                        args: vec![
                            ContractField {
                                name: s("to"),
                                ty: ContractType::Address,
                            },
                            ContractField {
                                name: s("amount"),
                                ty: ContractType::U128,
                            },
                        ]
                        .try_into()
                        .unwrap(),
                        return_type: ContractType::Void,
                    },
                    ContractSpecMethod {
                        name: s("metadata"),
                        args: vec![].try_into().unwrap(),
                        return_type: ContractType::Named {
                            named: s("TokenMeta"),
                        },
                    },
                ]
                .try_into()
                .unwrap(),
            },
            ContractSpecInterface {
                name: s("Admin"),
                vers: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
                methods: vec![ContractSpecMethod {
                    name: s("set_admin"),
                    args: vec![ContractField {
                        name: s("new_admin"),
                        ty: ContractType::Address,
                    }]
                    .try_into()
                    .unwrap(),
                    return_type: ContractType::Void,
                }]
                .try_into()
                .unwrap(),
            },
        ]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![
            ContractTypeDef::TypeDefEnum {
                enum_def: ContractEnumDef {
                    name: s("TokenError"),
                    variants: vec![
                        ContractEnumVariant {
                            name: s("InsufficientBalance"),
                            value: 0,
                        },
                        ContractEnumVariant {
                            name: s("Unauthorized"),
                            value: 1,
                        },
                    ]
                    .try_into()
                    .unwrap(),
                },
            },
            ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: s("TokenMeta"),
                    fields: vec![
                        ContractField {
                            name: s("name"),
                            ty: ContractType::String,
                        },
                        ContractField {
                            name: s("symbol"),
                            ty: ContractType::String,
                        },
                        ContractField {
                            name: s("decimals"),
                            ty: ContractType::U32,
                        },
                        ContractField {
                            name: s("icon"),
                            ty: ContractType::Option {
                                option_inner: Box::new(ContractType::Vector {
                                    vector: ContractVectorType {
                                        element: Box::new(ContractType::U8),
                                        max_len: 4096,
                                    },
                                }),
                            },
                        },
                    ]
                    .try_into()
                    .unwrap(),
                },
            },
            ContractTypeDef::TypeDefUnion {
                union_def: ContractUnionDef {
                    name: s("Event"),
                    cases: vec![
                        ContractUnionCase {
                            name: s("Transfer"),
                            discriminant: 0,
                            case_type:
                                ContractUnionCaseType::UnionCaseStruct {
                                    struct_fields: vec![
                                        ContractField {
                                            name: s("from"),
                                            ty: ContractType::Address,
                                        },
                                        ContractField {
                                            name: s("to"),
                                            ty: ContractType::Address,
                                        },
                                        ContractField {
                                            name: s("amount"),
                                            ty: ContractType::U128,
                                        },
                                    ]
                                    .try_into()
                                    .unwrap(),
                                },
                        },
                        ContractUnionCase {
                            name: s("Approval"),
                            discriminant: 1,
                            case_type: ContractUnionCaseType::UnionCaseTuple {
                                tuple: vec![
                                    ContractType::Address,
                                    ContractType::Address,
                                    ContractType::U128,
                                ]
                                .try_into()
                                .unwrap(),
                            },
                        },
                        ContractUnionCase {
                            name: s("Initialized"),
                            discriminant: 2,
                            case_type: ContractUnionCaseType::UnionCaseVoid,
                        },
                    ]
                    .try_into()
                    .unwrap(),
                },
            },
        ]
        .try_into()
        .unwrap(),
    };
    roundtrip(&spec);
}

// ---------------------------------------------------------------------------
// XDR → JSON → XDR round-trip (proves the two serialization formats agree)
// ---------------------------------------------------------------------------

#[test]
fn xdr_to_json_to_xdr_roundtrips() {
    use xdrs::cxs;

    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Iface"),
            vers: s("0.1.0"),
            throws: vec![exn(ContractType::Named { named: s("Err") })]
                .try_into()
                .unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("run"),
                args: vec![ContractField {
                    name: s("input"),
                    ty: ContractType::Vector {
                        vector: ContractVectorType {
                            element: Box::new(ContractType::U8),
                            max_len: u32::MAX,
                        },
                    },
                }]
                .try_into()
                .unwrap(),
                return_type: ContractType::Array {
                    array: ContractArrayType {
                        element: Box::new(ContractType::U8),
                        len: 32,
                    },
                },
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![ContractTypeDef::TypeDefEnum {
            enum_def: ContractEnumDef {
                name: s("Err"),
                variants: vec![ContractEnumVariant {
                    name: s("Bad"),
                    value: 1,
                }]
                .try_into()
                .unwrap(),
            },
        }]
        .try_into()
        .unwrap(),
    };

    // XDR → JSON → spec
    let xdr_bytes = cxs::encode(&spec).unwrap();
    let json = spec_to_json(&spec).unwrap();
    let from_json: ContractSpec = parse_spec_json(&json).unwrap();
    let from_xdr: ContractSpec = cxs::decode(&xdr_bytes).unwrap();

    assert_eq!(from_json, from_xdr);
    assert_eq!(from_json, spec);

    // And re-encode to XDR to confirm byte-level equality.
    let re_encoded = cxs::encode(&from_json).unwrap();
    assert_eq!(re_encoded, xdr_bytes);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn spec_with_method(name: &str, return_type: ContractType) -> ContractSpec {
    ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Iface"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s(name),
                args: vec![].try_into().unwrap(),
                return_type,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![].try_into().unwrap(),
    }
}

fn spec_with_typedef(type_def: ContractTypeDef) -> ContractSpec {
    let name = match &type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => &struct_def.name,
        ContractTypeDef::TypeDefEnum { enum_def } => &enum_def.name,
        ContractTypeDef::TypeDefUnion { union_def } => &union_def.name,
        _ => panic!("unknown variant"),
    };
    ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Iface"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("get"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::Named {
                    named: name.clone(),
                },
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
        type_defs: vec![type_def].try_into().unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

use groundhog_contract_spec::ContractEventDef;

#[test]
fn spec_with_events_roundtrips() {
    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Token"),
            vers: s("1.0.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![].try_into().unwrap(),
        }]
        .try_into()
        .unwrap(),
        events: vec![ContractEventDef {
            name: s("TransferEvent"),
            signature: [0xAB; 32].into(),
            topics: vec![
                ContractField {
                    name: s("from"),
                    ty: ContractType::Address,
                },
                ContractField {
                    name: s("to"),
                    ty: ContractType::Address,
                },
            ]
            .try_into()
            .unwrap(),
            fields: vec![ContractField {
                name: s("amount"),
                ty: ContractType::U128,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        type_defs: vec![].try_into().unwrap(),
    };
    roundtrip(&spec);

    let rendered = groundhog_spec_tool::render_spec_json(&spec);
    assert!(rendered.contains("\"events\""), "{rendered}");
    assert!(
        rendered.contains("\"name\":\"TransferEvent\""),
        "{rendered}"
    );
    assert!(
        rendered.contains(&format!("\"signature\":\"{}\"", "ab".repeat(32))),
        "{rendered}"
    );
}

#[test]
fn signed_token_types_roundtrip() {
    let signed_named = ContractType::SignedToken {
        signed_token_preimage:
            groundhog_contract_spec::ContractSignedTokenPreimage {
                preimage_type: Some(Box::new(ContractType::Named {
                    named: s("TransferAuth"),
                })),
            },
    };
    // A nested token whose child is an erased token (`None` preimage,
    // i.e. Signed<Opaque>).
    let signed_nested = ContractType::SignedToken {
        signed_token_preimage:
            groundhog_contract_spec::ContractSignedTokenPreimage {
                preimage_type: Some(Box::new(ContractType::SignedToken {
                    signed_token_preimage:
                        groundhog_contract_spec::ContractSignedTokenPreimage {
                            preimage_type: None,
                        },
                })),
            },
    };

    roundtrip(&ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Token"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("transfer"),
                args: vec![
                    ContractField {
                        name: s("auth"),
                        ty: signed_named,
                    },
                    ContractField {
                        name: s("amount"),
                        ty: ContractType::U64,
                    },
                ]
                .try_into()
                .unwrap(),
                return_type: signed_nested,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        type_defs: vec![ContractTypeDef::TypeDefStruct {
            struct_def: ContractStructDef {
                name: s("TransferAuth"),
                fields: vec![ContractField {
                    name: s("amount"),
                    ty: ContractType::U64,
                }]
                .try_into()
                .unwrap(),
            },
        }]
        .try_into()
        .unwrap(),
        events: vec![].try_into().unwrap(),
    });
}

#[test]
fn render_spec_json_renders_signed_token() {
    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Token"),
            vers: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("transfer"),
                args: vec![
                    ContractField {
                        name: s("auth"),
                        ty: ContractType::SignedToken {
                            signed_token_preimage:
                                groundhog_contract_spec::ContractSignedTokenPreimage {
                                    preimage_type: Some(Box::new(ContractType::U64)),
                                },
                        },
                    },
                    ContractField {
                        name: s("erased"),
                        ty: ContractType::SignedToken {
                            signed_token_preimage:
                                groundhog_contract_spec::ContractSignedTokenPreimage {
                                    preimage_type: None,
                                },
                        },
                    },
                ]
                .try_into()
                .unwrap(),
                return_type: ContractType::Void,
            }]
            .try_into()
            .unwrap(),
        }]
        .try_into()
        .unwrap(),
        type_defs: vec![].try_into().unwrap(),
        events: vec![].try_into().unwrap(),
    };

    let json = groundhog_spec_tool::render_spec_json(&spec);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["interfaces"][0]["methods"][0]["args"][0]["type"],
        serde_json::json!({"kind": "signed_token", "preimage": {"kind": "u64"}})
    );
    assert_eq!(
        value["interfaces"][0]["methods"][0]["args"][1]["type"],
        serde_json::json!({"kind": "signed_token", "preimage": null})
    );
}
