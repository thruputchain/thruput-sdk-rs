//! System functions
//!
//! Used by `#[contract]` macro.

use crate::sysresult::SysResult as Result;
use crate::sys;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::contract_panic;
use crate::internal::{WriteXDR, encode_xdr, EncodeBuf, get_encode_buf};
use core::mem::MaybeUninit;

pub fn contract_return(buf: &[u8]) -> ! {
    let revert = false;

    sys::raw_Return(buf, revert);
}

pub fn contract_exit(_code: i32) -> ! {
    sys::raw_Exit();
}

pub fn contract_revert() -> ! {
    sys::raw_Exit(/* crate::syscall::SyscallErrorCode::kDeterministicWasm as i32 */);
}

pub fn contract_abort(err: &impl WriteXDR) -> ! {
    let return_buf = encode_xdr(err).unwrap_or_else(|_| {
        contract_panic!("abort: failed uncoding return value")});
    let revert = true;

    sys::raw_Return(return_buf.as_ref(), revert);
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

#[cfg(any(feature = "mock", feature = "dynamic_bufs"))]
mod get_calldata {
    // Ask the VM for the size of the encoded arguments (the "calldata") before retrieving it into
    // a dynamically-resized array.

    use super::*;

    pub fn get_calldata() -> Result<EncodeBuf> {
        // Find out how much data the VM has waiting for us
        let len = get_calldata_len()?;

        // Get a buffer with at least that much spare capacity
        let mut calldata_buf = get_encode_buf(Some(len as usize));

        // Fetch the calldata from the VM
        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(calldata_buf.spare_capacity_mut(), &mut written)?;

        assert_eq!(written, len, "GetCalldata wrote fewer bytes than reported by GetCalldataLen");

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe { calldata_buf.set_len(written as usize); }

        Ok(calldata_buf)
    }
}

#[cfg(all(not(feature = "mock"), not(feature = "dynamic_bufs")))]
mod get_calldata {
    // We're using a static buffer

    use super::*;

    pub fn get_calldata() -> Result<EncodeBuf> {
        // Get a buffer to put the calldata in
        let mut calldata_buf = get_encode_buf(None);

        // Fetch the calldata from the VM, hoping our buffer is large enough
        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(calldata_buf.spare_capacity_mut(), &mut written)?;

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe { calldata_buf.set_len(written as usize); }

        // TODO: Panic if the calldata got truncated.  (Augment the syscall interface to find out,
        // or perhaps do get_calldata_len() if the buffer has been completely filled?)

        Ok(calldata_buf)
    }
}

pub use get_calldata::*;

pub fn return_result<RET>(result: &RET) -> !
where
    RET: WriteXDR
{
    // Encode the return value (into a possibly-shared buffer).
    let result_encoded = encode_xdr(result).unwrap_or_else(|_|
        contract_panic!("failed encoding contract-call result"));

    // Return the contract-call result to the VM
    contract_return(result_encoded.as_ref());
}
