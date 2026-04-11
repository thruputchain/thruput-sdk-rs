use groundhog_xdr::types::{BlockHeight, ContractAddress, SHA256Hash, Ed25519PublicKey, Ed25519Signature};

use crate::{ContractTypeInfo, ContractTypeRefSpec, view};

impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for BlockHeight {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for SHA256Hash {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: 32,
        }),
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for Ed25519PublicKey {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: 32,
        }),
        type_defs: view::ContractTypeDefsView::Empty,
    };
}

impl ContractTypeRefSpec for Ed25519Signature {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Vector(view::ContractVectorTypeView {
            element: &view::ContractTypeView::U8,
            max_len: 64,
        }),
        type_defs: view::ContractTypeDefsView::Empty,
    };
}