//! System functions
//!
//! Used by `#[contract]` macro.

use crate::sysresult::SysResult as Result;
use crate::sys;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::contract_panic;
use crate::serialize::{WriteXDR, encode_xdr};
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

#[cfg(all(not(feature = "mock"), feature = "dynamic_bufs"))]
mod get_calldata {
    // This strategy uses a dedicated, statically-allocated `Vec<u8>` (that is, a fat-pointer to a
    // dynamically-allocated buffer).
    //
    // A handle to this vec, with size reset to zero, is returned on each call to `get_calldata`.
    // Calling the function again before dropping the previous handle will result in a panic.

    use super::*;

    crate::static_vec! {
        fn get_static_calldata_buf(size_hint: Option<usize>) -> CalldataBuf;
    }

    pub fn get_calldata() -> Result<CalldataBuf> {
        // Find out how much data the VM has waiting for us
        let len = get_calldata_len()?;

        // Get a buffer with at least that many bytes reserved
        let mut calldata_buf = get_static_calldata_buf(Some(len as usize));

        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(calldata_buf.spare_capacity_mut(), &mut written)?;

        debug_assert_eq!(written, len, "GetCalldata wrote fewer bytes than reported by GetCalldataLen");

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe { calldata_buf.set_len(written as usize); }

        Ok(calldata_buf)
    }
}

#[cfg(all(not(feature = "mock"), not(feature = "dynamic_bufs")))]
mod get_calldata {
    // This strategy uses a dedicated, statically-allocated (and fixed-sized) buffer.
    //
    // A handle to this pointer is returned on each call to `get_calldata`.
    // Calling the function again before dropping the previous handle will result in a panic.

    use super::*;

    const CALLDATA_MAX: usize = 500;

    crate::static_mu_buf! {
        fn get_static_calldata_buf<CALLDATA_MAX>(size_hint: Option<usize>) -> CalldataBuf;
    }

    pub fn get_calldata() -> Result<CalldataBuf> {
        // Get a buffer to put the calldata in
        let mut calldata_buf = get_static_calldata_buf(None);

        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(calldata_buf.spare_capacity_mut(), &mut written)?;

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe { calldata_buf.set_len(written as usize); }

        Ok(calldata_buf)
    }
}

#[cfg(feature = "mock")]
mod get_calldata {
    // This strategy allows contract-invocation to be re-entrant, as is
    // often the case in the mock-testing environment.

    use super::*;
    use alloc::vec::Vec;

    pub fn get_calldata() -> Result<Vec<u8>> {
        // Find out how much data the VM has waiting for us
        let len = get_calldata_len()?;

        let mut calldata_buf = Vec::with_capacity(len as usize);

        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(calldata_buf.spare_capacity_mut(), &mut written)?;

        assert_eq!(written, len, "GetCalldata wrote fewer bytes than reported by GetCalldataLen");

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe { calldata_buf.set_len(written as usize); }

        Ok(calldata_buf)
    }
}

pub use get_calldata::*;

/* A previous, perhaps-very-slightly-faster but significantly-more-dangerous implementation
 *
pub fn get_calldata_static() -> Result<(*mut u8, RuntimeMemoryLength)> {
    const SZ: usize = 1000;
    static mut CALLDATA: MaybeUninit<[u8; SZ]> = MaybeUninit::uninit();

    let calldata_mu_ptr: *mut MaybeUninit<[u8; SZ]> = &raw mut CALLDATA;
    let calldata_ptr: &mut [u8; SZ] = unsafe {
        (*calldata_mu_ptr).assume_init_mut()
    };
    let calldata: &mut [u8] = &mut calldata_ptr[..];
    let mut written_len: RuntimeMemoryLength = 0;

    sys::raw_GetCalldata(calldata, &mut written_len)?;

    Ok((calldata.as_mut_ptr(), written_len))
}
*/

pub fn return_result<RET>(result: &RET) -> !
where
    RET: WriteXDR
{
    // TODO: hoist the buf out of encode_xdr() and omit the mutex

    let result_encoded = encode_xdr(result).unwrap_or_else(|_|
        contract_panic!("failed encoding contract-call result"));

    // Return the contract-call result to the VM
    contract_return(result_encoded.as_ref());
}
