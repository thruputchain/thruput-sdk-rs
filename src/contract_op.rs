//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::WriteXDR;
use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use core::mem::MaybeUninit;

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
    let return_buf = encode_return_value(ret).unwrap_or_else(|e| {
        panic!("contract abort: failed encoding return value: {}", e)
    });

    contract_abort(return_buf.as_ref());
}

pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    let return_buf = encode_return_value(ret).unwrap_or_else(|e| {
        panic!("contract return: failed encoding return value: {}", e)
    });

    contract_return(return_buf.as_ref());
}

// TODO: pull this configuration out to somewhere more accessible
const RETURN_MAX: usize = 500;

crate::static_mu_buf! {
    fn get_return_buf<RETURN_MAX>() -> EncodeBuf;
}

fn encode_return_value<RET>(result: &RET) -> xdrs::WriteResult<EncodeBuf>
where
    RET: WriteXDR + ?Sized,
{
    let mut buf = get_return_buf();

    let written =
        xdrs::serialize_txs::encode_into(result, buf.spare_capacity_mut())?;

    unsafe {
        buf.set_len(written);
    }

    Ok(buf)
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

// During guest execution, a contract can only be in one callframe at a time.
// So we can statically allocate a single buffer to hold invocation data.
#[cfg(all(not(feature = "dynamic_bufs"), not(feature = "mock")))]
mod get_calldata {
    // Use a static buffer

    use super::*;

    const CALLDATA_MAX: usize = 500;

    crate::static_mu_buf! {
        pub fn get_calldata_buf<CALLDATA_MAX>() -> Calldata;
    }

    pub fn get_calldata() -> Result<Calldata> {
        // Get a buffer to put the calldata in
        let mut calldata_buf = get_calldata_buf();

        // Fetch the calldata from the VM, hoping our buffer is large enough
        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(
            calldata_buf.spare_capacity_mut(),
            &mut written,
        )?;

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe {
            calldata_buf.set_len(written as usize);
        }

        // TODO: Panic if the calldata got truncated.  (Augment the syscall interface to find out,
        // or perhaps do get_calldata_len() if the buffer has been completely filled?)

        Ok(calldata_buf)
    }
}

// If we don't mind using liballoc, we can ask the VM for the size of the encoded arguments (the
// "calldata") before retrieving it into a dynamically-resized array.
#[cfg(any(feature = "dynamic_bufs", feature = "mock"))]
mod get_calldata {

    use super::*;

    // During mock testing, multiple contracts may execute in the same address-space and may call
    // each other. Thus, we need to allocate a fresh buffer to hold the invocation data for each
    // contract call.
    #[cfg(feature = "mock")]
    mod get_calldata_buf {
        use alloc::vec::Vec;

        pub type Calldata = Vec<u8>;

        pub fn get_calldata_buf(size: usize) -> Calldata {
            Vec::with_capacity(size)
        }
    }
    #[cfg(feature = "mock")]
    use get_calldata_buf::*;

    // Otherwise, use a static buffer and reallocate as needed to accommodate the needed size.
    #[cfg(not(feature = "mock"))]
    crate::static_vec! {
        pub fn get_calldata_buf(size) -> Calldata;
    }

    pub fn get_calldata() -> Result<Calldata> {
        // Find out how much data the VM has waiting for us
        let size = get_calldata_len()?;

        // Get a buffer with at least that much spare capacity
        let mut calldata_buf = get_calldata_buf(size as usize);

        // Fetch the calldata from the VM
        let mut written: RuntimeMemoryLength = 0;
        sys::uninit_GetCalldata(
            calldata_buf.spare_capacity_mut(),
            &mut written,
        )?;

        assert_eq!(
            written, size,
            "GetCalldata wrote fewer bytes than reported by GetCalldataLen"
        );

        // SAFETY: At least this many bytes of the buffer are now initialized
        unsafe {
            calldata_buf.set_len(written as usize);
        }

        Ok(calldata_buf)
    }
}

pub use get_calldata::*;
