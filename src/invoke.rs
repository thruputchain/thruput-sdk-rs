//! Cross-contract invocation functions.
//!
//! These functions are NO LONGER used by the [contract](macro@crate::contract) macro,
//! and remain until being phased out of use elsewhere.

use groundhog_xdr::contract::ContractEntrypointID;
use crate::internal::{
    EncodeBuf, MUBuf, ReadXDROwned, WriteXDR, decode_xdr, encode_xdr,
};
#[allow(unused_imports)]
pub use crate::invoke_error::*;
use crate::sys;
use crate::SysErr;
use crate::types::{ContractAddress, RuntimeMemoryLength};
use core::error::Error;

type CallData = EncodeBuf;
type ReturnData = EncodeBuf;

#[deprecated]
pub fn invoke<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> InvokeResult<RET>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
{
    // Encode the args
    let call_data = encode_xdr(args).map_err(|_| InvokeError::EncodeArgs)?;

    // Allocate a buffer for receiving the return data
    let mut return_data = MUBuf::new();

    // Invoke the method
    invoke_raw(address, entrypoint, gas_limit, &call_data, &mut return_data)
        .map_err(|syserr|
            if syserr.is_aborted() {
                InvokeError::AbortedWith(return_data.len() as u32)
            } else {
                InvokeError::Syscall(syserr)
            }
        )?;

    // Decode return data
    decode_xdr(return_data.as_ref())
        .map_err(|_| InvokeError::DecodeReturnVal)
}

// Invoke a client contract where we know the error type (which
// is produced on abort)
#[deprecated]
pub fn client_invoke<ARGS, RET, ERR>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> ClientResult<RET, ERR>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
    ERR: ReadXDROwned + WriteXDR + Error,
{
    // Encode the args
    let call_data = encode_xdr(args).map_err(|_| InvokeError::EncodeArgs)?;

    // Allocate a buffer for receiving the return data
    let mut return_data = MUBuf::new();

    // Invoke the method
    invoke_raw(address, entrypoint, gas_limit, &call_data, &mut return_data)
        .map_err(|syserr|
            if syserr.is_aborted() {
                decode_xdr(return_data.as_ref())
                    .map(ClientError::Aborted)
                    .unwrap_or_else(|_| ClientError::AbortDecodeFailed)
            } else {
                ClientError::Invoke(InvokeError::Syscall(syserr))
            }
        )?;

    // Decode return data
    decode_xdr(return_data.as_ref())
        .map_err(|_| ClientError::Invoke(InvokeError::DecodeReturnVal))
}

// At least this part won't be monomorphized multiple times
#[inline(never)]
#[allow(clippy::result_large_err)]
fn invoke_raw(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    call_data: &CallData,
    return_data: &mut ReturnData,
) -> core::result::Result<(), SysErr> {
    // Perform the cross-contract call

    let mut return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());

    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        call_data.as_ref(),
        0, // TODO(geoff): invoke flags go here
        &mut return_len,
    );

    match &result {
        Err(syserr) =>
            if !syserr.is_aborted() {
                // No return-data to process below
                return Err(*syserr);
            },
        _ => {}
    }

    // Ok or abort: get the return data from the VM ...

    let mut written_mu: core::mem::MaybeUninit<RuntimeMemoryLength> =
        core::mem::MaybeUninit::uninit();

    sys::uninit_GetReturnedBuffer(
        return_data.spare_capacity_mut(),
        &mut written_mu,
    )
    .unwrap_or_else(|_| panic!("couldn't get return data"));

    let written = unsafe { written_mu.assume_init() };

    if written != return_len {
        panic!(
            "unexpected return-data length from system call: \
                    written = {written}, \
                    return_len = {return_len}"
        );
    }

    // SAFETY: The VM reports above that this many bytes were written
    unsafe {
        return_data.set_len(written as usize);
    }

    result
}
