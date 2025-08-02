#![allow(non_snake_case)]

use crate::result::SysResult as Result;
use crate::sys;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use core::mem::MaybeUninit;

pub fn get_caller() -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetMsgSender(&mut out_addr)?;
    Ok(unsafe { out_addr.assume_init() })
}

pub fn get_current_address() -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)?;
    Ok(unsafe { out_addr.assume_init() })
}

pub fn get_calldata_len() -> Result<RuntimeMemoryLength> {
    let mut out: RuntimeMemoryLength = 0;
    sys::raw_GetCalldataLen(&mut out)?;
    Ok(out)
}

pub fn get_calldata_sized<const N: usize>() -> Result<[u8; N]> {
    let mut calldata_mu = [::core::mem::MaybeUninit::<u8>::uninit(); N];

    let mut calldata_len: RuntimeMemoryLength = 0;

    sys::uninit_GetCalldata(&mut calldata_mu, &mut calldata_len)?;

    if calldata_len != N as u32 {
        panic!("Unexpected calldata size");
    }

    let calldata: [u8; N] =
        unsafe { calldata_mu.map(|c| MaybeUninit::assume_init(c)) };
    Ok(calldata)
}

pub fn contract_return(buf: &[u8], do_revert: bool) -> ! {
    sys::raw_Return(buf, do_revert);
}

pub fn contract_exit(_code: i32) -> ! {
    sys::raw_Exit();
}

pub fn contract_revert() -> ! {
    sys::raw_Exit(/* crate::syscall::SyscallErrorCode::kDeterministicWasm as i32 */);
}

pub fn get_source_transaction_hash() -> Result<SHA256Hash> {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash)?;
    Ok(unsafe { out_hash.assume_init() })
}

#[cfg(feature = "alloc")]
pub use get_calldata_alloc::*;

#[cfg(feature = "alloc")]
mod get_calldata_alloc {
    use crate::result::SysResult as Result;
    use crate::sys;
    use crate::types::RuntimeMemoryLength;
    use alloc::boxed::Box;

    pub fn get_calldata() -> Result<Box<[u8]>> {
        let len: RuntimeMemoryLength = super::get_calldata_len()?;

        let mut vec = Box::<[u8]>::new_uninit_slice(len as usize);

        let mut written_len: RuntimeMemoryLength = 0;

        sys::uninit_GetCalldata(&mut vec, &mut written_len)?;

        if written_len != len {
            panic!("Unexpected calldata size");
        }

        Ok(unsafe { vec.assume_init() })
    }
}
