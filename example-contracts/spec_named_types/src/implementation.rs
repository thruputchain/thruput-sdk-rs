use sdk::__contract_spec::{
    ContractEnumDefView, ContractEnumVariantView, ContractFieldView,
    ContractStructDefView, ContractTypeDefView, ContractTypeSpec,
    ContractTypeView, ContractUnionCaseView, ContractUnionDefView,
};
use sdk::*;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct Payload {
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum Choice {
    Left = 1,
    Right = 2,
}

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
#[repr(i32)]
pub enum Envelope {
    Empty = 0,
    Wrapped { payload: Payload } = 1,
}

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

impl ContractTypeSpec for Choice {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Enum(ContractEnumDefView {
            name: "Choice",
            variants: &[
                ContractEnumVariantView {
                    name: "Left",
                    value: 1,
                },
                ContractEnumVariantView {
                    name: "Right",
                    value: 2,
                },
            ],
        });
}

impl ContractTypeSpec for Envelope {
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Union(ContractUnionDefView {
            name: "Envelope",
            cases: &[
                ContractUnionCaseView {
                    name: "Empty",
                    discriminant: 0,
                    case_type: None,
                },
                ContractUnionCaseView {
                    name: "Wrapped",
                    discriminant: 1,
                    case_type: Some(ContractTypeView::Named("Payload")),
                },
            ],
        });
}

#[derive(Default)]
pub struct SpecNamedTypesContract;

#[contract(version = "0.3.0")]
impl SpecNamedTypesContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }

    pub fn choose(choice: Choice) -> Choice {
        choice
    }

    pub fn wrap(flag: bool, payload: Payload) -> Envelope {
        if flag {
            Envelope::Wrapped { payload }
        } else {
            Envelope::Empty
        }
    }
}

#[cfg(target_family = "wasm")]
#[allow(dead_code)]
mod raw_typedefs {
    use super::{Choice, Envelope, Payload};
    use sdk::__contract_spec::{ContractTypeDefView, ContractTypeSpec};

    const PAYLOAD_FRAG: ContractTypeDefView<'static> =
        <Payload as ContractTypeSpec>::TYPE_DEF;
    const PAYLOAD_LEN: usize =
        sdk::internal::const_xdr_spec_typedef_fragment_len(&PAYLOAD_FRAG);
    const PAYLOAD_RAW: [u8; PAYLOAD_LEN] =
        sdk::internal::const_xdr_spec_typedef_fragment_bytes::<PAYLOAD_LEN>(
            &PAYLOAD_FRAG,
        );

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_PAYLOAD_TYPE_DEF: [u8; PAYLOAD_LEN] = PAYLOAD_RAW;

    const CHOICE_FRAG: ContractTypeDefView<'static> =
        <Choice as ContractTypeSpec>::TYPE_DEF;
    const CHOICE_LEN: usize =
        sdk::internal::const_xdr_spec_typedef_fragment_len(&CHOICE_FRAG);
    const CHOICE_RAW: [u8; CHOICE_LEN] =
        sdk::internal::const_xdr_spec_typedef_fragment_bytes::<CHOICE_LEN>(
            &CHOICE_FRAG,
        );

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_CHOICE_TYPE_DEF: [u8; CHOICE_LEN] = CHOICE_RAW;

    const ENVELOPE_FRAG: ContractTypeDefView<'static> =
        <Envelope as ContractTypeSpec>::TYPE_DEF;
    const ENVELOPE_LEN: usize =
        sdk::internal::const_xdr_spec_typedef_fragment_len(&ENVELOPE_FRAG);
    const ENVELOPE_RAW: [u8; ENVELOPE_LEN] =
        sdk::internal::const_xdr_spec_typedef_fragment_bytes::<ENVELOPE_LEN>(
            &ENVELOPE_FRAG,
        );

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_ENVELOPE_TYPE_DEF: [u8; ENVELOPE_LEN] = ENVELOPE_RAW;
}
