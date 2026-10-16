//! System functions: Contract deployment

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::types::{ContractAddress, SHA256Hash};
use core::mem::MaybeUninit;

pub fn get_deploy_address(
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetDeployAddress(None, nonce, contract_hash, &mut out_addr)
        .or_syscall_throw(SyscallNo::kGetDeployAddress);

    unsafe { out_addr.assume_init() }
}

pub fn get_deploy_address_for_other(
    other_address: &ContractAddress,
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetDeployAddress(
        Some(other_address),
        nonce,
        contract_hash,
        &mut out_addr,
    )
    .or_syscall_throw(SyscallNo::kGetDeployAddress);

    unsafe { out_addr.assume_init() }
}

pub fn deploy_contract(
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_DeployContract(nonce, contract_hash, &mut out_addr)
        .or_syscall_throw(SyscallNo::kDeployContract);

    unsafe { out_addr.assume_init() }
}
