//! System functions: Contract deployment

use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::{ContractAddress, SHA256Hash};
use core::mem::MaybeUninit;

pub fn get_deploy_address(
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetDeployAddress(None, nonce, contract_hash, &mut out_addr)?;

    Ok(unsafe { out_addr.assume_init() })
}

pub fn get_deploy_address_for_other(
    other_address: &ContractAddress,
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetDeployAddress(
        Some(other_address),
        nonce,
        contract_hash,
        &mut out_addr,
    )?;

    Ok(unsafe { out_addr.assume_init() })
}

pub fn deploy_contract(
    nonce: &SHA256Hash,
    contract_hash: &SHA256Hash,
) -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    crate::sys::uninit_DeployContract(nonce, contract_hash, &mut out_addr)?;

    Ok(unsafe { out_addr.assume_init() })
}
