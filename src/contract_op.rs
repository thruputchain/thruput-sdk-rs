//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::WriteXDR;
use crate::internal::MUBuf;
use crate::sys::{self, SyscallNo};
use crate::syscall_abort2;
use crate::sysresult::SysResult as Result;
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
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
pub fn contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized,
{
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
        .unwrap_or_else(|e| syscall_abort2(SyscallNo::kGetMsgSender, e));

    unsafe { out_addr.assume_init() }
}

pub fn get_source_transaction_hash() -> SHA256Hash {
    let mut out_hash: MaybeUninit<SHA256Hash> =
        MaybeUninit::<SHA256Hash>::uninit();
    sys::uninit_GetSourceTransactionHash(&mut out_hash).unwrap_or_else(|e| {
        syscall_abort2(SyscallNo::kGetSourceTransactionHash, e)
    });
    unsafe { out_hash.assume_init() }
}

pub fn get_current_address() -> ContractAddress {
    let mut out_addr: MaybeUninit<ContractAddress> =
        MaybeUninit::<ContractAddress>::uninit();
    sys::uninit_GetCurrentAddress(&mut out_addr)
        .unwrap_or_else(|e| syscall_abort2(SyscallNo::kGetCurrentAddress, e));
    unsafe { out_addr.assume_init() }
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
    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    sys::uninit_GetCalldata(calldata.spare_capacity_mut(), &mut written)?;
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
pub fn bare_get_calldata() -> Calldata {
    get_calldata().unwrap_or_else(|syserr| {
        syscall_abort2(SyscallNo::kGetCalldata, syserr)
    })
}
