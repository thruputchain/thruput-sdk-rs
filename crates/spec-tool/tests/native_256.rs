use groundhog_contract_spec::{
    ContractEventDefView, ContractFieldView, ContractSpecMethodView,
    ContractType, ContractTypeRefSpec, SpecFragmentView,
    SpecInterfaceAnchorView, SpecInterfaceFragmentView,
    SpecMethodFragmentView,
};
use groundhog_spec_tool::{canonicalize_spec, parse_raw_fragments};
use sdk::{I256, U256};

const INTERFACE: &str = "WideIntegers";
const VERSION: &str = "1.0.0";
const EVENT: &str = "WideIntegerEvent";

const U256_TYPE: groundhog_contract_spec::ContractTypeView<'static> =
    <U256 as ContractTypeRefSpec>::INFO.ty;
const I256_TYPE: groundhog_contract_spec::ContractTypeView<'static> =
    <I256 as ContractTypeRefSpec>::INFO.ty;

const FRAGMENTS: &[SpecFragmentView<'static>] = &[
    SpecFragmentView::Interface(SpecInterfaceFragmentView {
        interface_name: INTERFACE,
        interface_version: VERSION,
        declared_by: "native_256_if@0.1::Iface",
        throws: &[],
        emits: &[EVENT],
    }),
    SpecFragmentView::Method(SpecMethodFragmentView {
        interface_name: INTERFACE,
        interface_version: VERSION,
        declared_by: "native_256_if@0.1::Iface",
        method: ContractSpecMethodView {
            name: "convert",
            args: &[ContractFieldView {
                name: "unsigned",
                ty: U256_TYPE,
            }],
            return_type: I256_TYPE,
        },
    }),
    SpecFragmentView::Event(ContractEventDefView {
        name: EVENT,
        signature: [0xA5; 32],
        topics: &[
            ContractFieldView {
                name: "unsigned",
                ty: U256_TYPE,
            },
            ContractFieldView {
                name: "signed",
                ty: I256_TYPE,
            },
        ],
        fields: &[],
    }),
    SpecFragmentView::InterfaceAnchor(SpecInterfaceAnchorView {
        interface_name: INTERFACE,
        interface_version: VERSION,
        declared_by: "native_256_if@0.1::Iface",
    }),
];

#[test]
fn raw_fragments_assemble_native_256_types_without_typedefs() {
    let mut raw = Vec::new();
    for fragment in FRAGMENTS {
        raw.extend(xdrs::txs::encode(fragment).unwrap());
    }

    let fragments = parse_raw_fragments(&raw).unwrap();
    let spec = canonicalize_spec(&fragments).unwrap();
    let method = &spec.interfaces[0].methods[0];
    let event = &spec.events[0];

    assert_eq!(
        (
            &method.args[0].ty,
            &method.return_type,
            &event.topics[0].ty,
            &event.topics[1].ty,
            spec.type_defs.len(),
        ),
        (
            &ContractType::U256,
            &ContractType::I256,
            &ContractType::U256,
            &ContractType::I256,
            0,
        )
    );
}
