//! System functions: Contract deployment
//!
//! [`CodeHash::deploy`] is the typed form: it takes a [`DeployNonce`] and
//! returns an [`Address`](crate::Address). The free functions here take and
//! return the untyped hashes and address.

mod typed;

pub use typed::{CodeHash, DeployNonce};

use crate::re_throw::OrSyscallThrow;
use crate::sys::{self, SyscallNo};
use crate::types::{ContractAddress, SHA256Hash};
use core::mem::MaybeUninit;

/// The address [`deploy_contract`] gives for `nonce` and `contract_hash` when
/// the current contract calls it. Untyped form of
/// [`CodeHash::deploy_address`].
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

/// The address [`deploy_contract`] gives for `nonce` and `contract_hash` when
/// `other_address` calls it. Untyped form of
/// [`CodeHash::deploy_address_for`].
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

/// Deploy the contract with code `contract_hash` from the current contract.
/// Untyped form of [`CodeHash::deploy`].
///
/// Both arguments are a `&SHA256Hash`, so the compiler does not catch them
/// swapped. Prefer the typed form.
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
