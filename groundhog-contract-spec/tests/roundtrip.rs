use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumDefView, ContractEnumVariant,
    ContractEnumVariantView, ContractField, ContractFieldView, ContractSpec,
    ContractSpecInterface, ContractSpecInterfaceView, ContractSpecMethod,
    ContractSpecMethodView, ContractSpecView, ContractStructDefView,
    ContractType, ContractTypeDef, ContractTypeDefView, ContractTypeSpec,
    ContractTypeView, ContractUnionCaseView, ContractUnionDefView,
    SpecFragment, SpecFragmentView, SpecInterfaceFragment,
    SpecInterfaceFragmentView, SpecMethodFragment, SpecMethodFragmentView,
};
use xdrs::cxs::{decode, encode};
use xdrs::txs;

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
            name: "Named".to_string(),
            version: "0.1.0".to_string(),
            throws: vec![ContractType::Named("DemoError".to_string())],
            methods: vec![ContractSpecMethod {
                name: "ping".to_string(),
                args: vec![ContractField {
                    name: "value".to_string(),
                    ty: ContractType::U32,
                }],
                return_type: ContractType::Void,
            }],
        }],
        type_defs: vec![ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".to_string(),
            variants: vec![ContractEnumVariant {
                name: "Nope".to_string(),
                value: 7,
            }],
        })],
    };

    let encoded = encode(&spec).unwrap();
    let decoded: ContractSpec = decode(&encoded).unwrap();
    assert_eq!(decoded, spec);
}

#[test]
fn borrowed_views_convert_to_owned_model() {
    let named_payload = ContractTypeView::Named("Payload");
    let named_error = ContractTypeView::Named("DemoError");
    let field_views = [ContractFieldView {
        name: "count",
        ty: ContractTypeView::U32,
    }];
    let variant_views = [ContractEnumVariantView {
        name: "Nope",
        value: 7,
    }];
    let case_views = [ContractUnionCaseView {
        name: "Wrapped",
        discriminant: 1,
        case_type: Some(named_payload),
    }];
    let method_args = [ContractFieldView {
        name: "payload",
        ty: named_payload,
    }];
    let option_named_error = ContractTypeView::Option(&named_error);
    let vec_named_payload = ContractTypeView::Vec(&named_payload);
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
        ContractTypeView::Named("Payload"),
    ];
    let methods = [ContractSpecMethodView {
        name: "ping",
        args: &method_args,
        return_type: ContractTypeView::Tuple(&tuple_items),
    }];
    let interface_throws = [named_error];
    let interfaces = [ContractSpecInterfaceView {
        name: "Named",
        version: "0.1.0",
        throws: &interface_throws,
        methods: &methods,
    }];
    let type_defs = [
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "Payload",
            fields: &field_views,
        }),
        ContractTypeDefView::Enum(ContractEnumDefView {
            name: "DemoError",
            variants: &variant_views,
        }),
        ContractTypeDefView::Union(ContractUnionDefView {
            name: "Envelope",
            cases: &case_views,
        }),
    ];

    let owned = ContractSpec::from(ContractSpecView {
        interfaces: &interfaces,
        type_defs: &type_defs,
    });

    assert_eq!(owned.interfaces.len(), 1);
    assert_eq!(owned.type_defs.len(), 3);
    assert_eq!(
        owned.interfaces[0].throws,
        vec![ContractType::Named("DemoError".to_string())]
    );
    assert_eq!(
        owned.interfaces[0].methods[0].return_type,
        ContractType::Tuple(vec![
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
            ContractType::Option(Box::new(ContractType::Named(
                "DemoError".to_string(),
            ))),
            ContractType::Vec(Box::new(ContractType::Named(
                "Payload".to_string(),
            ))),
            ContractType::Named("Payload".to_string()),
        ])
    );
}

#[test]
fn fragment_and_view_conversions_roundtrip() {
    let interface = SpecInterfaceFragmentView {
        package_name: "demo",
        interface_name: "Named",
        interface_version: "0.1.0",
        throws: &[ContractTypeView::Named("DemoError")],
    };
    let method = SpecMethodFragmentView {
        package_name: "demo",
        interface_name: "Named",
        interface_version: "0.1.0",
        method: ContractSpecMethodView {
            name: "ping",
            args: &[ContractFieldView {
                name: "payload",
                ty: ContractTypeView::Named("Payload"),
            }],
            return_type: ContractTypeView::Void,
        },
    };
    let type_def = ContractTypeDefView::Union(ContractUnionDefView {
        name: "Envelope",
        cases: &[ContractUnionCaseView {
            name: "Empty",
            discriminant: 0,
            case_type: None,
        }],
    });

    let interface_fragment =
        SpecFragment::from(SpecFragmentView::Interface(interface));
    let method_fragment = SpecFragment::from(SpecFragmentView::Method(method));
    let type_def_fragment =
        SpecFragment::from(SpecFragmentView::TypeDef(type_def));

    assert!(matches!(interface_fragment, SpecFragment::Interface(_)));
    assert!(matches!(method_fragment, SpecFragment::Method(_)));
    assert!(matches!(
        type_def_fragment,
        SpecFragment::TypeDef(ContractTypeDef::Union(_))
    ));

    assert_eq!(
        ContractTypeDef::from(type_def),
        ContractTypeDef::Union(groundhog_contract_spec::ContractUnionDef {
            name: "Envelope".to_string(),
            cases: vec![groundhog_contract_spec::ContractUnionCase {
                name: "Empty".to_string(),
                discriminant: 0,
                case_type: None,
            }],
        })
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
fn borrowed_fragment_views_have_const_txs_writers() {
    const THROWS: [ContractTypeView<'static>; 1] =
        [ContractTypeView::Named("DemoError")];
    const FRAGMENT: SpecInterfaceFragmentView<'static> =
        SpecInterfaceFragmentView {
            package_name: "demo",
            interface_name: "Named",
            interface_version: "0.1.0",
            throws: &THROWS,
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
fn typedef_fragment_stream_helpers_match_runtime_xdr() {
    const TYPE_DEFS: &[Option<ContractTypeDefView<'static>>] = &[
        Some(ContractTypeDefView::Struct(ContractStructDefView {
            name: "Payload",
            fields: &[ContractFieldView {
                name: "count",
                ty: ContractTypeView::U32,
            }],
        })),
        None,
        Some(ContractTypeDefView::Enum(ContractEnumDefView {
            name: "Choice",
            variants: &[ContractEnumVariantView {
                name: "Left",
                value: 1,
            }],
        })),
    ];
    const LEN: usize =
        groundhog_contract_spec::const_txs_typedef_fragment_stream_len(
            TYPE_DEFS,
        );
    const RAW: [u8; LEN] =
        groundhog_contract_spec::const_txs_typedef_fragment_stream_bytes::<LEN>(
            TYPE_DEFS,
        );

    let expected = [
        txs::encode(&SpecFragmentView::TypeDef(TYPE_DEFS[0].unwrap()))
            .unwrap(),
        txs::encode(&SpecFragmentView::TypeDef(TYPE_DEFS[2].unwrap()))
            .unwrap(),
    ]
    .concat();

    assert_eq!(&RAW, expected.as_slice());
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
    assert_xdr_tag(&ContractType::Option(Box::new(ContractType::Void)), 11);
    assert_xdr_tag(&ContractType::Vec(Box::new(ContractType::Void)), 12);
    assert_xdr_tag(&ContractType::Tuple(vec![ContractType::Void]), 13);
    assert_xdr_tag(&ContractType::Named("DemoError".to_string()), 14);

    assert_xdr_tag(
        &ContractTypeDef::Struct(groundhog_contract_spec::ContractStructDef {
            name: "Payload".to_string(),
            fields: vec![],
        }),
        0,
    );
    assert_xdr_tag(
        &ContractTypeDef::Enum(groundhog_contract_spec::ContractEnumDef {
            name: "Choice".to_string(),
            variants: vec![],
        }),
        1,
    );
    assert_xdr_tag(
        &ContractTypeDef::Union(groundhog_contract_spec::ContractUnionDef {
            name: "Envelope".to_string(),
            cases: vec![],
        }),
        2,
    );

    assert_xdr_tag(
        &SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".to_string(),
            interface_name: "Named".to_string(),
            interface_version: "0.1.0".to_string(),
            throws: vec![],
        }),
        0,
    );
    assert_xdr_tag(
        &SpecFragment::Method(SpecMethodFragment {
            package_name: "demo".to_string(),
            interface_name: "Named".to_string(),
            interface_version: "0.1.0".to_string(),
            method: ContractSpecMethod {
                name: "ping".to_string(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        1,
    );
    assert_xdr_tag(
        &SpecFragment::TypeDef(ContractTypeDef::Enum(
            groundhog_contract_spec::ContractEnumDef {
                name: "Choice".to_string(),
                variants: vec![],
            },
        )),
        2,
    );
}

#[test]
fn contract_type_spec_helpers_are_derived_from_type_def() {
    struct Payload;

    impl ContractTypeSpec for Payload {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: "Payload",
                fields: &[ContractFieldView {
                    name: "count",
                    ty: ContractTypeView::U32,
                }],
            });
    }

    assert_eq!(Payload::TYPE_DEF.symbol(), "Payload");
    assert_eq!(groundhog_contract_spec::symbol::<Payload>(), "Payload");
    assert!(matches!(
        groundhog_contract_spec::type_ref::<Payload>(),
        ContractTypeView::Named("Payload")
    ));
    assert!(matches!(Payload::TYPE_DEF, ContractTypeDefView::Struct(_)));
    assert_eq!(
        groundhog_contract_spec::type_ref::<Payload>(),
        Payload::TYPE_DEF.type_ref()
    );
    assert_eq!(
        groundhog_contract_spec::maybe_type_def::<Payload>(),
        Some(Payload::TYPE_DEF)
    );
}

#[test]
fn contract_type_ref_helpers_cover_primitives_and_composites() {
    assert_eq!(
        groundhog_contract_spec::type_ref::<u32>(),
        ContractTypeView::U32
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Option<u32>>(),
        ContractTypeView::Option(&ContractTypeView::U32)
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<Vec<u32>>(),
        ContractTypeView::Vec(&ContractTypeView::U32)
    );
    assert_eq!(
        groundhog_contract_spec::type_ref::<(u32, bool)>(),
        ContractTypeView::Tuple(&[
            ContractTypeView::U32,
            ContractTypeView::Bool,
        ])
    );
    assert_eq!(groundhog_contract_spec::maybe_type_def::<u32>(), None);
    assert_eq!(
        groundhog_contract_spec::maybe_type_def::<Option<u32>>(),
        None
    );
    assert_eq!(groundhog_contract_spec::maybe_type_def::<Vec<u32>>(), None);
    assert_eq!(
        groundhog_contract_spec::maybe_type_def::<(u32, bool)>(),
        None
    );
}
