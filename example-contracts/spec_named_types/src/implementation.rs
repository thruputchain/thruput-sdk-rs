extern crate alloc;

use alloc::vec::Vec;
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

pub type PayloadPair = (Payload, Option<Payload>);
pub type Byte = u8;
pub type Blob = Vec<Byte>;
pub type Hash = [Byte; 32];
pub type BlobEnvelope = (Blob, Hash);

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

impl sdk::internal::ContractAbiAllocEnabled for Payload {}

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

impl sdk::internal::ContractAbiAllocEnabled for Choice {}

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
    const TYPE_DEFS: sdk::__contract_spec::ContractTypeDefsView<'static> =
        sdk::__contract_spec::ContractTypeDefsView::Many(&[
            sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
        sdk::__contract_spec::type_defs::<Payload>(),
        ]);
}

impl sdk::internal::ContractAbiAllocEnabled for Envelope {}

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

    pub fn echo_pair(payloads: PayloadPair) -> PayloadPair {
        payloads
    }

    pub fn echo_blob(blob: Blob) -> Blob {
        blob
    }

    pub fn echo_hash(hash: Hash) -> Hash {
        hash
    }

    pub fn echo_blob_envelope(envelope: BlobEnvelope) -> BlobEnvelope {
        envelope
    }
}

#[cfg(target_family = "wasm")]
#[allow(dead_code)]
mod raw_typedefs {
    use super::{Choice, Envelope, Payload};
    use sdk::__contract_spec::{ContractTypeSpec, SpecFragmentView};

    const PAYLOAD_FRAG: SpecFragmentView<'static> = SpecFragmentView::TypeDef(
        <Payload as ContractTypeSpec>::TYPE_DEF,
    );
    const PAYLOAD_LEN: usize = PAYLOAD_FRAG.const_txs_size();
    const PAYLOAD_RAW: [u8; PAYLOAD_LEN] = {
        let mut w = sdk::__xdrs::txs::ConstWriter::<PAYLOAD_LEN>::new();
        PAYLOAD_FRAG.const_write_txs(&mut w);
        w.into_bytes()
    };

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_PAYLOAD_TYPE_DEF: [u8; PAYLOAD_LEN] = PAYLOAD_RAW;

    const CHOICE_FRAG: SpecFragmentView<'static> = SpecFragmentView::TypeDef(
        <Choice as ContractTypeSpec>::TYPE_DEF,
    );
    const CHOICE_LEN: usize = CHOICE_FRAG.const_txs_size();
    const CHOICE_RAW: [u8; CHOICE_LEN] = {
        let mut w = sdk::__xdrs::txs::ConstWriter::<CHOICE_LEN>::new();
        CHOICE_FRAG.const_write_txs(&mut w);
        w.into_bytes()
    };

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_CHOICE_TYPE_DEF: [u8; CHOICE_LEN] = CHOICE_RAW;

    const ENVELOPE_FRAG: SpecFragmentView<'static> =
        SpecFragmentView::TypeDef(<Envelope as ContractTypeSpec>::TYPE_DEF);
    const ENVELOPE_LEN: usize = ENVELOPE_FRAG.const_txs_size();
    const ENVELOPE_RAW: [u8; ENVELOPE_LEN] = {
        let mut w = sdk::__xdrs::txs::ConstWriter::<ENVELOPE_LEN>::new();
        ENVELOPE_FRAG.const_write_txs(&mut w);
        w.into_bytes()
    };

    #[cfg_attr(
        target_family = "wasm",
        unsafe(link_section = "groundhogspecrawv0")
    )]
    pub static RAW_ENVELOPE_TYPE_DEF: [u8; ENVELOPE_LEN] = ENVELOPE_RAW;
}
