//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::WriteXDR;
use crate::internal::MUBuf;
use crate::internal::encode_xdr;
use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use core::mem::MaybeUninit;

const CALLDATA_MAX: usize = 500;

pub fn contract_return(buf: &[u8]) -> ! {
    let revert = false;

    sys::raw_Return(buf, revert);
}

pub fn contract_abort(buf: &[u8]) -> ! {
    let revert = true;

    sys::raw_Return(buf, revert);
}

pub fn contract_exit(_code: i32) -> ! {
    sys::raw_Exit();
}

pub fn contract_revert() -> ! {
    sys::raw_Exit(/* crate::syscall::SyscallErrorCode::kDeterministicWasm as i32 */);
}

pub fn contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    let return_buf = encode_xdr(ret).unwrap_or_else(|e| {
        panic!("contract abort: failed encoding return value: {}", e)
    });

    contract_abort(return_buf.as_ref());
}

pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    let return_buf = encode_xdr(ret).unwrap_or_else(|e| {
        panic!("contract return: failed encoding return value: {}", e)
    });

    contract_return(return_buf.as_ref());
}

pub fn get_caller() -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetMsgSender(&mut out_addr)?;
    Ok(unsafe { out_addr.assume_init() })
}

pub fn get_source_transaction_hash() -> Result<SHA256Hash> {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash)?;
    Ok(unsafe { out_hash.assume_init() })
}

pub fn get_current_address() -> Result<ContractAddress> {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)?;
    Ok(unsafe { out_addr.assume_init() })
}

#[inline]
pub fn get_calldata_len() -> Result<RuntimeMemoryLength> {
    let mut out: RuntimeMemoryLength = 0;
    sys::raw_GetCalldataLen(&mut out)?;
    Ok(out)
}

pub type Calldata = MUBuf<CALLDATA_MAX>;

pub fn get_calldata() -> Result<Calldata> {
    // Get a buffer to put the calldata in
    let mut calldata: Calldata = MUBuf::new();

    // Fetch the calldata from the VM, hoping our buffer is large enough
    let mut written: RuntimeMemoryLength = 0;
    sys::uninit_GetCalldata(calldata.spare_capacity_mut(), &mut written)?;

    // SAFETY: At least this many bytes of the buffer are now initialized
    unsafe {
        calldata.set_len(written as usize);
    }

    // TODO: Panic if the calldata got truncated.  (Augment the syscall interface to find out,
    // or perhaps do get_calldata_len() if the buffer has been completely filled?)

    Ok(calldata)
}
