//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::internal::MUBuf;
use crate::sys::{self, SysResult, SyscallNo};
use crate::syscall_throw;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::xdr::*;
use core::mem::MaybeUninit;

pub(crate) const CALLDATA_MAX: usize = 500;

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

pub fn contract_return_empty() -> ! {
    contract_return(&[])
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    {
        std::println!("Throwing exception {:?}", ret);
        std::println!("Location: {}", core::panic::Location::caller());
        std::println!(
            "Backtrace:\n{}",
            crate::debug_context::Backtrace::capture()
        );
    }
    crate::call::abort_encoded(ret)
}

#[inline]
pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
    crate::call::return_encoded(ret)
}

pub fn get_caller() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();

    sys::uninit_GetMsgSender(&mut out_addr)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetMsgSender, e));

    unsafe { out_addr.assume_init() }
}

pub fn get_source_transaction_hash() -> SHA256Hash {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash).unwrap_or_else(|e| {
        syscall_throw(SyscallNo::kGetSourceTransactionHash, e)
    });
    unsafe { out_hash.assume_init() }
}

pub fn get_current_address() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetCurrentAddress, e));
    unsafe { out_addr.assume_init() }
}

pub fn try_get_calldata_len() -> SysResult<RuntimeMemoryLength> {
    let mut out: RuntimeMemoryLength = 0;
    let mut _auth_token_count: u32 = 0;
    sys::raw_GetCalldataLen(&mut out, &mut _auth_token_count)?;
    Ok(out)
}

pub fn get_calldata_len() -> RuntimeMemoryLength {
    try_get_calldata_len()
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kGetCalldataLen, e))
}

pub type Calldata = MUBuf<CALLDATA_MAX>;

pub fn try_get_calldata() -> SysResult<Calldata> {
    // Get a buffer to put the calldata in
    let mut calldata: Calldata = MUBuf::new();

    // Fetch the calldata from the VM, hoping our buffer is large enough
    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut _auth_token_written: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();
    sys::uninit_GetCalldata(
        calldata.spare_capacity_mut(),
        &mut written,
        &mut [],
        &mut _auth_token_written,
    )?;
    let written = unsafe { written.assume_init() };

    // SAFETY: At least this many bytes of the buffer are now initialized
    unsafe {
        calldata.set_len(written as usize);
    }

    // TODO: Panic if the calldata got truncated.  (Augment the syscall interface to find out,
    // or perhaps do get_calldata_len() if the buffer has been completely filled?)

    Ok(calldata)
}

// This should probably become `get_calldata`, once the existing clients
// of that function are adapted to non-result return.
pub fn get_calldata() -> Calldata {
    try_get_calldata().unwrap_or_else(|syserr| {
        syscall_throw(SyscallNo::kGetCalldata, syserr)
    })
}
