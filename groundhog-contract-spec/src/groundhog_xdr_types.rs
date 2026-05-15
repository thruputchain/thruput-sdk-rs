use groundhog_xdr::{
    SyscallNo,
    contract::ContractEntrypointID,
    types::{
        BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
        SHA256Hash,
    },
};

#[cfg(feature = "syscalls")]
use groundhog_xdr::syscalls::SysErr;

use crate::{ContractTypeInfo, ContractTypeRefSpec, view};

impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
    };
}

impl ContractTypeRefSpec for BlockHeight {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I64,
    };
}

impl ContractTypeRefSpec for SHA256Hash {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 32,
        }),
    };
}

impl ContractTypeRefSpec for Ed25519PublicKey {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 32,
        }),
    };
}

impl ContractTypeRefSpec for Ed25519Signature {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 64,
        }),
    };
}

impl ContractTypeRefSpec for ContractEntrypointID {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: &view::ContractTypeView::U8,
            len: 8,
        }),
    };
}

impl ContractTypeRefSpec for SyscallNo {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
    };
}

#[cfg(feature = "syscalls")]
impl ContractTypeRefSpec for SysErr {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
    };
}
