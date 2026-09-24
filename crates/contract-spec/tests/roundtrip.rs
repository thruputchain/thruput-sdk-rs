use groundhog_contract_spec::{
    ContractArrayType, ContractArrayTypeView, ContractEnumDef,
    ContractEnumDefView, ContractEnumVariant, ContractEnumVariantView,
    ContractEventDef, ContractEventDefView, ContractExceptionType,
    ContractExceptionTypeView, ContractField, ContractFieldView,
    ContractSignedTokenPreimage, ContractSpec, ContractSpecId,
    ContractSpecIdView, ContractSpecInterface, ContractSpecInterfaceView,
    ContractSpecMethod, ContractSpecMethodView, ContractSpecString,
    ContractSpecView, ContractStructDefView, ContractType, ContractTypeDef,
    ContractTypeDefView, ContractTypeInfo, ContractTypeRefSpec,
    ContractTypeSpec, ContractTypeView, ContractUnionCase,
    ContractUnionCaseType, ContractUnionCaseTypeView, ContractUnionCaseView,
    ContractUnionDef, ContractUnionDefView, ContractVectorType,
    ContractVectorTypeView, SpecFragment, SpecFragmentView,
    SpecInterfaceFragment, SpecInterfaceFragmentView, SpecMethodFragment,
    SpecMethodFragmentView,
};
use xdrs::cxs::{decode, encode};
use xdrs::txs;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

/// The segments every fixture id below carries, apart from `declared`.
///
/// `idv` and `id` must describe the same id: this file roundtrips a view
/// through its owned form and compares, so a mismatch here would read as an
/// encoding bug rather than a fixture one.
const FIXTURE_CRATE: &str = "testcrate";
const FIXTURE_MAJOR: &str = "0";
const FIXTURE_MODULE: &str = "testcrate";

/// A fixture id, owned.
fn id(declared: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: s(FIXTURE_CRATE),
        major: s(FIXTURE_MAJOR),
        module_path: s(FIXTURE_MODULE),
        declared: s(declared),
    }
}

/// The same fixture id, as the const view.
const fn idv(declared: &'static str) -> ContractSpecIdView<'static> {
    ContractSpecIdView {
        crate_name: FIXTURE_CRATE,
        major: FIXTURE_MAJOR,
        module_path: FIXTURE_MODULE,
        declared,
    }
}

fn assert_xdr_tag<T: xdrs::WriteXDR>(value: &T, expected: i32) {
    let encoded = encode(value).unwrap();
    assert!(!encoded.is_empty());
    // cxs encodes these small discriminants as a single leading tag byte.
    assert_eq!(encoded[0], expected as u8);
}

#[test]
fn contract_spec_roundtrips_through_xdr() {
    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s("Named"),
            vers: s("0.1.0"),
            throws: vec![ContractExceptionType {
                selector: 0xDEAD,
                ty: ContractType::Named {
                    named: id("DemoError"),
                },
            }]
            .try_into()
            .unwrap(),
            methods: vec![ContractSpecMethod {
                name: s("ping"),
                args: vec![ContractField {
                    name: s("value"),
                    ty: ContractType::U32,
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
        type_defs: vec![ContractTypeDef::TypeDefEnum {
            enum_def: ContractEnumDef {
                name: id("DemoError"),
                variants: vec![ContractEnumVariant {
                    name: s("Nope"),
                    value: 7,
                }]
                .try_into()
                .unwrap(),
            },
        }]
        .try_into()
        .unwrap(),
        events: vec![ContractEventDef {
            name: id("TransferEvent"),
            signature: [0xAB; 32].into(),
            topics: vec![ContractField {
                name: s("from"),
                ty: ContractType::Address,
            }]
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
    };

    let encoded = encode(&spec).unwrap();
    let decoded: ContractSpec = decode(&encoded).unwrap();
    assert_eq!(decoded, spec);
}

#[test]
fn borrowed_views_convert_to_owned_model() {
    let named_payload = ContractTypeView::Named(idv("Payload"));
    let named_error = ContractTypeView::Named(idv("DemoError"));
    let field_views = [ContractFieldView {
        name: "count",
        ty: ContractTypeView::U32,
    }];
    let variant_views = [ContractEnumVariantView {
        name: "Nope",
        value: 7,
    }];
    let wrapped_fields = [ContractFieldView {
        name: "payload",
        ty: named_payload,
    }];
    let tuple_types = [named_payload];
    let case_views = [
        ContractUnionCaseView {
            name: "Wrapped",
            discriminant: 1,
            case_type: ContractUnionCaseTypeView::Struct(&wrapped_fields),
        },
        ContractUnionCaseView {
            name: "Packed",
            discriminant: 2,
            case_type: ContractUnionCaseTypeView::Tuple(&tuple_types),
        },
    ];
    let method_args = [ContractFieldView {
        name: "payload",
        ty: named_payload,
    }];
    let option_named_error = ContractTypeView::Option(&named_error);
    let vec_named_payload = ContractTypeView::Vector(ContractVectorTypeView {
        element: &named_payload,
        max_len: u32::MAX,
    });
    let tuple_items = [
        ContractTypeView::Void,
        ContractTypeView::Bool,
        ContractTypeView::U32,
        ContractTypeView::I32,
        ContractTypeView::U64,
        ContractTypeView::I64,
        ContractTypeView::U128,
        ContractTypeView::I128,
        ContractTypeView::String,
        ContractTypeView::Bytes,
        ContractTypeView::Address,
        option_named_error,
        vec_named_payload,
        ContractTypeView::Named(idv("Payload")),
        ContractTypeView::U8,
        ContractTypeView::Array(ContractArrayTypeView {
            element: &ContractTypeView::U8,
            len: 32,
        }),
        ContractTypeView::SignedToken(Some(&named_payload)),
        ContractTypeView::SignedToken(None),
    ];
    let methods = [ContractSpecMethodView {
        name: "ping",
        args: &method_args,
        return_type: ContractTypeView::Tuple(&tuple_items),
    }];
    let interface_throws = [ContractExceptionTypeView {
        selector: 0xDEAD,
        ty: named_error,
    }];
    let interfaces = [ContractSpecInterfaceView {
        name: "Named",
        version: "0.1.0",
        throws: &interface_throws,
        methods: &methods,
    }];
    let type_defs = [
        ContractTypeDefView::Struct(ContractStructDefView {
            name: idv("Payload"),
            fields: &field_views,
        }),
        ContractTypeDefView::Enum(ContractEnumDefView {
            name: idv("DemoError"),
            variants: &variant_views,
        }),
        ContractTypeDefView::Union(ContractUnionDefView {
            name: idv("Envelope"),
            cases: &case_views,
        }),
    ];
    let event_topics = [ContractFieldView {
        name: "from",
        ty: ContractTypeView::Address,
    }];
    let event_fields = [ContractFieldView {
        name: "payload",
        ty: named_payload,
    }];
    let events = [ContractEventDefView {
        name: idv("TransferEvent"),
        signature: [0xAB; 32],
        topics: &event_topics,
        fields: &event_fields,
    }];

    let owned = ContractSpec::from(ContractSpecView {
        interfaces: &interfaces,
        type_defs: &type_defs,
        events: &events,
    });

    assert_eq!(owned.interfaces.len(), 1);
    assert_eq!(owned.type_defs.len(), 3);
    assert_eq!(
        owned.events.as_slice(),
        &[ContractEventDef {
            name: id("TransferEvent"),
            signature: [0xAB; 32].into(),
            topics: vec![ContractField {
                name: s("from"),
                ty: ContractType::Address,
            }]
            .try_into()
            .unwrap(),
            fields: vec![ContractField {
                name: s("payload"),
                ty: ContractType::Named {
                    named: id("Payload")
                },
            }]
            .try_into()
            .unwrap(),
        }]
    );
    assert_eq!(
        owned.interfaces[0].throws.as_slice(),
        &[ContractExceptionType {
            selector: 0xDEAD,
            ty: ContractType::Named {
                named: id("DemoError")
            },
        }]
    );
    assert_eq!(
        owned.interfaces[0].methods[0].return_type,
        ContractType::Tuple {
            tuple: vec![
                ContractType::Void,
                ContractType::Bool,
                ContractType::U32,
                ContractType::I32,
                ContractType::U64,
                ContractType::I64,
                ContractType::U128,
                ContractType::I128,
                ContractType::String,
                ContractType::Bytes,
                ContractType::Address,
                ContractType::Option {
                    option_inner: Box::new(ContractType::Named {
                        named: id("DemoError"),
                    }),
                },
                ContractType::Vector {
                    vector: ContractVectorType {
                        element: Box::new(ContractType::Named {
                            named: id("Payload"),
                        }),
                        max_len: u32::MAX,
                    },
                },
                ContractType::Named {
                    named: id("Payload")
                },
                ContractType::U8,
                ContractType::Array {
                    array: ContractArrayType {
                        element: Box::new(ContractType::U8),
                        len: 32,
                    },
                },
                ContractType::SignedToken {
                    signed_token_preimage: ContractSignedTokenPreimage {
                        preimage_type: Some(Box::new(ContractType::Named {
                            named: id("Payload"),
                        })),
                    },
                },
                ContractType::SignedToken {
                    signed_token_preimage: ContractSignedTokenPreimage {
                        preimage_type: None,
                    },
                },
            ]
            .try_into()
            .unwrap()
        }
    );
    assert_eq!(
        owned.type_defs[2],
        ContractTypeDef::TypeDefUnion {
            union_def: ContractUnionDef {
                name: id("Envelope"),
                cases: vec![
                    ContractUnionCase {
                        name: s("Wrapped"),
                        discriminant: 1,
                        case_type: ContractUnionCaseType::UnionCaseStruct {
                            struct_fields: vec![ContractField {
                                name: s("payload"),
                                ty: ContractType::Named {
                                    named: id("Payload")
                                },
                            },]
                            .try_into()
                            .unwrap(),
                        },
                    },
                    ContractUnionCase {
                        name: s("Packed"),
                        discriminant: 2,
                        case_type: ContractUnionCaseType::UnionCaseTuple {
                            tuple: vec![ContractType::Named {
                                named: id("Payload")
                            },]
                            .try_into()
                            .unwrap(),
                        },
                    },
                ]
                .try_into()
                .unwrap(),
            }
        }
    );
}

#[test]
fn fragment_and_view_conversions_roundtrip() {
    let interface_throws = [ContractExceptionTypeView {
        selector: 0xDEAD,
        ty: ContractTypeView::Named(idv("DemoError")),
    }];
    let interface = SpecInterfaceFragmentView {
        interface_name: "Named",
        interface_version: "0.1.0",
        throws: &interface_throws,
        emits: &[],
    };
    let method = SpecMethodFragmentView {
        interface_name: "Named",
        interface_version: "0.1.0",
        method: ContractSpecMethodView {
            name: "ping",
            args: &[ContractFieldView {
                name: "payload",
                ty: ContractTypeView::Named(idv("Payload")),
            }],
            return_type: ContractTypeView::Void,
        },
    };
    let type_def = ContractTypeDefView::Union(ContractUnionDefView {
        name: idv("Envelope"),
        cases: &[ContractUnionCaseView {
            name: "Empty",
            discriminant: 0,
            case_type: ContractUnionCaseTypeView::Void,
        }],
    });

    let interface_fragment =
        SpecFragment::from(SpecFragmentView::Interface(interface));
    let method_fragment = SpecFragment::from(SpecFragmentView::Method(method));
    let type_def_fragment =
        SpecFragment::from(SpecFragmentView::TypeDef(type_def));

    assert!(matches!(
        interface_fragment,
        SpecFragment::SpecInterface { .. }
    ));
    assert!(matches!(method_fragment, SpecFragment::SpecMethod { .. }));
    assert!(matches!(
        type_def_fragment,
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefUnion { .. }
        }
    ));

    assert_eq!(
        ContractTypeDef::from(type_def),
        ContractTypeDef::TypeDefUnion {
            union_def: groundhog_contract_spec::ContractUnionDef {
                name: id("Envelope"),
                cases: vec![groundhog_contract_spec::ContractUnionCase {
                    name: s("Empty"),
                    discriminant: 0,
                    case_type: ContractUnionCaseType::UnionCaseVoid,
                }]
                .try_into()
                .unwrap(),
            }
        }
    );

    let encoded = encode(&interface_fragment).unwrap();
    let decoded: SpecFragment = decode(&encoded).unwrap();
    assert_eq!(decoded, interface_fragment);

    let encoded = encode(&method_fragment).unwrap();
    let decoded: SpecFragment = decode(&encoded).unwrap();
    assert_eq!(decoded, method_fragment);

    let encoded = encode(&type_def_fragment).unwrap();
    let decoded: SpecFragment = decode(&encoded).unwrap();
    assert_eq!(decoded, type_def_fragment);
}

#[test]
fn borrowed_fragment_views_roundtrip_into_owned_fragments_via_txs() {
    let interface_throws = [ContractExceptionTypeView {
        selector: 0xDEAD,
        ty: ContractTypeView::Named(idv("DemoError")),
    }];
    let interface = SpecFragmentView::Interface(SpecInterfaceFragmentView {
        interface_name: "Named",
        interface_version: "0.3.0",
        throws: &interface_throws,
        emits: &[],
    });
    let method = SpecFragmentView::Method(SpecMethodFragmentView {
        interface_name: "Named",
        interface_version: "0.3.0",
        method: ContractSpecMethodView {
            name: "ping",
            args: &[ContractFieldView {
                name: "payload",
                ty: ContractTypeView::Named(idv("Payload")),
            }],
            return_type: ContractTypeView::Void,
        },
    });
    let type_def = SpecFragmentView::TypeDef(ContractTypeDefView::Struct(
        ContractStructDefView {
            name: idv("Payload"),
            fields: &[ContractFieldView {
                name: "count",
                ty: ContractTypeView::U32,
            }],
        },
    ));
    let event = SpecFragmentView::Event(ContractEventDefView {
        name: idv("TransferEvent"),
        signature: [0xAB; 32],
        topics: &[ContractFieldView {
            name: "from",
            ty: ContractTypeView::Address,
        }],
        fields: &[ContractFieldView {
            name: "payload",
            ty: ContractTypeView::Named(idv("Payload")),
        }],
    });

    let encoded = txs::encode(&interface).unwrap();
    let decoded: SpecFragment = txs::decode(&encoded).unwrap();
    assert_eq!(decoded, SpecFragment::from(interface));

    let encoded = txs::encode(&method).unwrap();
    let decoded: SpecFragment = txs::decode(&encoded).unwrap();
    assert_eq!(decoded, SpecFragment::from(method));

    let encoded = txs::encode(&type_def).unwrap();
    let decoded: SpecFragment = txs::decode(&encoded).unwrap();
    assert_eq!(decoded, SpecFragment::from(type_def));

    let encoded = txs::encode(&event).unwrap();
    let decoded: SpecFragment = txs::decode(&encoded).unwrap();
    assert_eq!(decoded, SpecFragment::from(event));
    assert!(matches!(
        SpecFragment::from(event),
        SpecFragment::SpecEvent { .. }
    ));
}

#[test]
fn borrowed_fragment_views_have_const_txs_writers() {
    const THROWS: [ContractExceptionTypeView<'static>; 1] =
        [ContractExceptionTypeView {
            selector: 0xDEAD,
            ty: ContractTypeView::Named(idv("DemoError")),
        }];
    const FRAGMENT: SpecInterfaceFragmentView<'static> =
        SpecInterfaceFragmentView {
            interface_name: "Named",
            interface_version: "0.1.0",
            throws: &THROWS,
            emits: &[],
        };
    const RAW: [u8; FRAGMENT.const_txs_size()] = {
        let mut w = txs::ConstWriter::<{ FRAGMENT.const_txs_size() }>::new();
        FRAGMENT.const_write_txs(&mut w);
        w.into_bytes()
    };

    let runtime = txs::encode(&FRAGMENT).unwrap();
    assert_eq!(&RAW, runtime.as_slice());
}

#[test]
fn enum_numbering_and_fragment_tags_are_pinned() {
    assert_xdr_tag(&ContractType::Void, 0);
    assert_xdr_tag(&ContractType::Bool, 1);
    assert_xdr_tag(&ContractType::U32, 2);
    assert_xdr_tag(&ContractType::I32, 3);
    assert_xdr_tag(&ContractType::U64, 4);
    assert_xdr_tag(&ContractType::I64, 5);
    assert_xdr_tag(&ContractType::U128, 6);
    assert_xdr_tag(&ContractType::I128, 7);
    assert_xdr_tag(&ContractType::String, 8);
    assert_xdr_tag(&ContractType::Bytes, 9);
    assert_xdr_tag(&ContractType::Address, 10);
    assert_xdr_tag(
        &ContractType::Option {
            option_inner: Box::new(ContractType::Void),
        },
        11,
    );
    assert_xdr_tag(
        &ContractType::Vector {
            vector: ContractVectorType {
                element: Box::new(ContractType::Void),
                max_len: u32::MAX,
            },
        },
        12,
    );
    assert_xdr_tag(
        &ContractType::Tuple {
            tuple: vec![ContractType::Void].try_into().unwrap(),
        },
        13,
    );
    assert_xdr_tag(
        &ContractType::Named {
            named: id("DemoError"),
        },
        14,
    );
    assert_xdr_tag(&ContractType::U8, 15);
    assert_xdr_tag(
        &ContractType::Array {
            array: ContractArrayType {
                element: Box::new(ContractType::U8),
                len: 32,
            },
        },
        16,
    );
    assert_xdr_tag(
        &ContractType::SignedToken {
            signed_token_preimage: ContractSignedTokenPreimage {
                preimage_type: Some(Box::new(ContractType::U64)),
            },
        },
        17,
    );
    assert_xdr_tag(
        &ContractType::SignedToken {
            signed_token_preimage: ContractSignedTokenPreimage {
                preimage_type: None,
            },
        },
        17,
    );

    assert_xdr_tag(
        &ContractTypeDef::TypeDefStruct {
            struct_def: groundhog_contract_spec::ContractStructDef {
                name: id("Payload"),
                fields: vec![].try_into().unwrap(),
            },
        },
        0,
    );
    assert_xdr_tag(
        &ContractTypeDef::TypeDefEnum {
            enum_def: groundhog_contract_spec::ContractEnumDef {
                name: id("Choice"),
                variants: vec![].try_into().unwrap(),
            },
        },
        1,
    );
    assert_xdr_tag(
        &ContractTypeDef::TypeDefUnion {
            union_def: groundhog_contract_spec::ContractUnionDef {
                name: id("Envelope"),
                cases: vec![].try_into().unwrap(),
            },
        },
        2,
    );

    assert_xdr_tag(
        &SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
                emits: vec![].try_into().unwrap(),
            },
        },
        0,
    );
    assert_xdr_tag(
        &SpecFragment::SpecMethod {
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
        1,
    );
    assert_xdr_tag(
        &SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefEnum {
                enum_def: groundhog_contract_spec::ContractEnumDef {
                    name: id("Choice"),
                    variants: vec![].try_into().unwrap(),
                },
            },
        },
        2,
    );

    let event_def = ContractEventDef {
        name: id("TransferEvent"),
        signature: [0xAB; 32].into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    };
    assert_xdr_tag(
        &SpecFragment::SpecEvent {
            event_def: event_def.clone(),
        },
        4,
    );
    assert_xdr_tag(&SpecFragment::ExportedSpecEvent { event_def }, 5);
}

#[test]
fn signed_token_view_matches_owned_wire_encoding() {
    // The view's `SignedToken(&inner)` must encode byte-identically to the
    // owned model's `SignedToken { signed_token_preimage }` node.
    let view = ContractTypeView::SignedToken(Some(&ContractTypeView::Named(
        idv("TransferAuth"),
    )));
    let owned = ContractType::from(view);
    assert_eq!(
        owned,
        ContractType::SignedToken {
            signed_token_preimage: ContractSignedTokenPreimage {
                preimage_type: Some(Box::new(ContractType::Named {
                    named: id("TransferAuth"),
                })),
            },
        }
    );
    assert_eq!(encode(&view).unwrap(), encode(&owned).unwrap());

    // An erased (opaque) preimage: `None` on both sides of the model.
    let opaque = ContractTypeView::SignedToken(None);
    let owned_opaque = ContractType::from(opaque);
    assert_eq!(
        owned_opaque,
        ContractType::SignedToken {
            signed_token_preimage: ContractSignedTokenPreimage {
                preimage_type: None,
            },
        }
    );
    assert_eq!(encode(&opaque).unwrap(), encode(&owned_opaque).unwrap());

    // Nested child tokens spec recursively.
    let nested = ContractTypeView::SignedToken(Some(
        &ContractTypeView::SignedToken(None),
    ));
    let owned_nested = ContractType::from(nested);
    assert_eq!(encode(&nested).unwrap(), encode(&owned_nested).unwrap());
}

#[test]
fn contract_type_spec_helpers_are_derived_from_type_def() {
    struct Payload;

    impl ContractTypeSpec for Payload {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: idv("Payload"),
                fields: &[ContractFieldView {
                    name: "count",
                    ty: ContractTypeView::U32,
                }],
            });
    }

    impl ContractTypeRefSpec for Payload {
        const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
            ty: <Self as ContractTypeSpec>::TYPE_DEF.type_ref(),
        };
    }

    assert_eq!(Payload::TYPE_DEF.symbol(), idv("Payload"));
    assert_eq!(Payload::TYPE_DEF.symbol().declared, "Payload");
    assert_eq!(groundhog_contract_spec::symbol::<Payload>(), idv("Payload"));
    assert_eq!(
        groundhog_contract_spec::type_ref::<Payload>(),
        ContractTypeView::Named(idv("Payload")),
    );
    assert!(matches!(Payload::TYPE_DEF, ContractTypeDefView::Struct(_)));
    assert_eq!(
        groundhog_contract_spec::type_ref::<Payload>(),
        Payload::TYPE_DEF.type_ref()
    );
}

#[test]
fn contract_type_ref_helpers_cover_primitives_and_composites() {
    assert_eq!(
        groundhog_contract_spec::type_ref::<u8>(),
        ContractTypeView::U8
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<u32>(),
        ContractTypeView::U32
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Option<u32>>(),
        ContractTypeView::Option(&ContractTypeView::U32)
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Vec<u8>>(),
        ContractTypeView::Vector(ContractVectorTypeView {
            element: &ContractTypeView::U8,
            max_len: u32::MAX,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<&[u8]>(),
        ContractTypeView::Vector(ContractVectorTypeView {
            element: &ContractTypeView::U8,
            max_len: u32::MAX,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<&str>(),
        ContractTypeView::String
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Vec<u32>>(),
        ContractTypeView::Vector(ContractVectorTypeView {
            element: &ContractTypeView::U32,
            max_len: u32::MAX,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<[u8; 32]>(),
        ContractTypeView::Array(ContractArrayTypeView {
            element: &ContractTypeView::U8,
            len: 32,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<[bool; 4]>(),
        ContractTypeView::Array(ContractArrayTypeView {
            element: &ContractTypeView::Bool,
            len: 4,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<(u32, bool)>(),
        ContractTypeView::Tuple(&[
            ContractTypeView::U32,
            ContractTypeView::Bool,
        ])
    );
}

#[test]
fn alias_byte_collections_resolve_via_trait_layer() {
    type Byte = u8;
    type Blob = Vec<Byte>;
    type Hash = [Byte; 32];

    assert_eq!(
        groundhog_contract_spec::type_ref::<Blob>(),
        ContractTypeView::Vector(ContractVectorTypeView {
            element: &ContractTypeView::U8,
            max_len: u32::MAX,
        })
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Hash>(),
        ContractTypeView::Array(ContractArrayTypeView {
            element: &ContractTypeView::U8,
            len: 32,
        })
    );
}

#[test]
fn interface_fragment_emits_roundtrips() {
    let throws = [ContractExceptionTypeView {
        selector: 0xDEAD,
        ty: ContractTypeView::Named(idv("DemoError")),
    }];
    let view = SpecFragmentView::Interface(SpecInterfaceFragmentView {
        interface_name: "Token",
        interface_version: "0.1.0",
        throws: &throws,
        emits: &[idv("TransferEvent"), idv("ApprovalEvent")],
    });
    let encoded = txs::encode(&view).unwrap();
    let decoded: SpecFragment = txs::decode(&encoded).unwrap();
    let SpecFragment::SpecInterface {
        ref interface_fragment,
    } = decoded
    else {
        panic!("wrong variant");
    };
    assert_eq!(
        interface_fragment.emits.to_vec(),
        vec![id("TransferEvent"), id("ApprovalEvent")]
    );
    assert_eq!(interface_fragment.throws.len(), 1);
    assert_eq!(interface_fragment.throws[0].selector, 0xDEAD);
    // Owned conversion path agrees:
    let owned = SpecFragment::from(view);
    assert_eq!(owned, decoded);
}

#[test]
fn contract_type_ref_helpers_cover_groundhog_xdr_builtins() {
    assert_eq!(
        groundhog_contract_spec::type_ref::<
            groundhog_xdr::types::ContractAddress,
        >(),
        ContractTypeView::Address
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<groundhog_xdr::types::BlockHeight>(
        ),
        ContractTypeView::I64
    );
}
