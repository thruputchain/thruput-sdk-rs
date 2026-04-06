use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumDefView, ContractEnumVariant,
    ContractEnumVariantView, ContractField, ContractFieldView, ContractSpec,
    ContractSpecInterface, ContractSpecInterfaceView, ContractSpecMethod,
    ContractSpecMethodView, ContractSpecView, ContractType, ContractTypeDef,
    ContractTypeDefView, ContractTypeSpec, ContractTypeView, ContractStructDefView,
    ContractUnionCaseView, ContractUnionDefView,
};
use xdrs::cxs::{decode, encode};

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
    let field_views = [ContractFieldView {
        name: "count",
        ty: ContractTypeView::U32,
    }];
    let variant_views = [
        ContractEnumVariantView {
            name: "Nope",
            value: 7,
        },
    ];
    let case_views = [ContractUnionCaseView {
        name: "Wrapped",
        discriminant: 1,
        case_type: Some(ContractTypeView::Named("Payload")),
    }];
    let method_args = [ContractFieldView {
        name: "payload",
        ty: ContractTypeView::Named("Payload"),
    }];
    let tuple_items = [
        ContractTypeView::U32,
        ContractTypeView::Named("Payload"),
    ];
    let methods = [ContractSpecMethodView {
        name: "ping",
        args: &method_args,
        return_type: ContractTypeView::Tuple(&tuple_items),
    }];
    let interface_throws = [ContractTypeView::Named("DemoError")];
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
    assert_eq!(owned.interfaces[0].throws, vec![ContractType::Named("DemoError".to_string())]);
    assert_eq!(
        owned.interfaces[0].methods[0].return_type,
        ContractType::Tuple(vec![
            ContractType::U32,
            ContractType::Named("Payload".to_string()),
        ])
    );
}

#[test]
fn contract_type_spec_constants_can_be_exposed() {
    struct Payload;

    impl ContractTypeSpec for Payload {
        const SYMBOL: &'static str = "Payload";
        const TYPE_REF: ContractTypeView<'static> = ContractTypeView::Named("Payload");
        const TYPE_DEF: ContractTypeDefView<'static> = ContractTypeDefView::Struct(
            ContractStructDefView {
                name: "Payload",
                fields: &[ContractFieldView {
                    name: "count",
                    ty: ContractTypeView::U32,
                }],
            },
        );
    }

    assert_eq!(Payload::SYMBOL, "Payload");
    assert!(matches!(Payload::TYPE_REF, ContractTypeView::Named("Payload")));
    assert!(matches!(Payload::TYPE_DEF, ContractTypeDefView::Struct(_)));
}
