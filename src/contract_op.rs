//! System functions
//!
//! The functions are used by the [`contract`](macro@crate::contract) macro,
//! and are also available for lower-level code.

use crate::internal::MUBuf;
use crate::sys::{self, SysResult, SyscallNo};
use crate::types::{ContractAddress, RuntimeMemoryLength, SHA256Hash};
use crate::xdr::*;
use crate::{
    CollectAuthHandles, FaultString, HandlingError, library_throw,
    syscall_throw,
};
use core::mem::MaybeUninit;

pub(crate) const CALLDATA_MAX: usize = 500;

pub fn contract_return(buf: &[u8], auth_token_ids: &[u32]) -> ! {
    let revert = false;

    let auth_token_ids_bytes =
        crate::call::auth_handles_as_bytes(auth_token_ids);

    sys::raw_Return(buf, revert, auth_token_ids_bytes);
}

pub fn contract_abort(buf: &[u8]) -> ! {
    let revert = true;

    sys::raw_Return(buf, revert, &[]);
}

pub fn contract_exit(_code: i32) -> ! {
    sys::raw_Exit();
}

pub fn contract_revert() -> ! {
    sys::raw_Exit();
}

pub fn contract_return_empty() -> ! {
    contract_return(&[], &[]);
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    {
        std::println!();
        std::println!("  Throwing exception {:?}", ret);
        std::println!("  Location: {}", core::panic::Location::caller());
        std::println!(
            "  Backtrace:\n{}",
            crate::debug_context::Backtrace::capture()
        );
    }

    _contract_abort_encoded(ret)
}

// Codepath for aborts in client-side codegen
#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn client_contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + core::fmt::Debug,
{
    #[cfg(all(feature = "debugging", feature = "std"))]
    {
        std::println!();
        std::println!("  Throwing exception {:?}", ret);
        std::println!(
            "  Interface location: {}",
            core::panic::Location::caller()
        );
        std::println!(
            "  Backtrace:\n{}",
            crate::debug_context::Backtrace::capture()
        );
    }

    _contract_abort_encoded(ret)
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
fn _contract_abort_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + core::fmt::Debug,
{
    // Use match to preserve call tracking
    let return_buf = match crate::call::contract_call_encode(ret) {
        Ok(buf) => buf,
        Err(e) => {
            // XXX: potential recursion
            library_throw(HandlingError::EncodingException(
                FaultString::via_display(&e),
            ))
        }
    };

    contract_abort(return_buf.as_ref());
}

#[inline]
#[cfg_attr(feature = "debugging", track_caller)]
pub fn contract_return_encoded<RET>(ret: &RET) -> !
where
    RET: WriteXDR + ?Sized + CollectAuthHandles,
{
    let return_buf =
        crate::call::contract_call_encode(ret).unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingException(
                FaultString::via_display(&e),
            ))
        });
    let auth_handles = crate::call::collect_auth_handle_bytes(ret)
        .unwrap_or_else(|e| {
            library_throw(HandlingError::EncodingException(
                FaultString::from_display(e),
            ));
        });

    contract_return(return_buf.as_ref(), auth_handles.as_slice());
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
#[deprecated(
    note = "use get_calldata_with_auth_tokens() for Signed token support"
)]
pub fn get_calldata() -> Calldata {
    try_get_calldata().unwrap_or_else(|syserr| {
        syscall_throw(SyscallNo::kGetCalldata, syserr)
    })
}

pub type AuthTokenIdBuf = heapless::Vec<u32, 32>;

pub fn get_calldata_with_auth_tokens() -> (Calldata, AuthTokenIdBuf) {
    try_get_calldata_with_auth_tokens().unwrap_or_else(|syserr| {
        syscall_throw(SyscallNo::kGetCalldata, syserr)
    })
}

pub fn try_get_calldata_with_auth_tokens()
-> SysResult<(Calldata, AuthTokenIdBuf)> {
    let mut calldata: Calldata = MUBuf::new();
    // 32 u32s * 4 bytes each
    let mut auth_token_buf: MUBuf<{ 32 * 4 }> = MUBuf::new();

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    let mut auth_token_written: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();

    sys::uninit_GetCalldata(
        calldata.spare_capacity_mut(),
        &mut written,
        auth_token_buf.spare_capacity_mut(),
        &mut auth_token_written,
    )?;

    let written = unsafe { written.assume_init() };
    unsafe { calldata.set_len(written as usize) };

    let auth_written = unsafe { auth_token_written.assume_init() };
    unsafe { auth_token_buf.set_len(auth_written as usize) };

    // Convert bytes to u32s using native byte order (wasm is little-endian)
    let mut auth_ids = AuthTokenIdBuf::new();
    for chunk in auth_token_buf.as_ref().chunks_exact(4) {
        let id = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        auth_ids.push(id).expect("too many auth token IDs");
    }

    Ok((calldata, auth_ids))
}
