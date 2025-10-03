//! System functions: invoke, client_invoke

use crate::sys;
use crate::types::{ ContractAddress, RuntimeMemoryLength };
use crate::contract::ContractEntrypointID;
use crate::contract_panic;
use alloc::{vec, vec::Vec};
use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};
use core::error::Error;

pub use crate::invoke_error::*;

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
    let call_data = encode(args).or(Err(InvokeError::EncodeArgs))?;

    // Invoke the method
    let return_buf =
        invoke_inner(address, entrypoint, gas_limit, call_data.as_slice())?;

    // Deserialize the buffer into a return value ...
    let return_val = decode(return_buf.as_slice())
        .map_err(|_| InvokeError::DecodeReturnVal(return_buf))?;

    Ok(return_val)
}

// At least this part won't be monomorphized multiple times
#[inline(never)]
fn invoke_inner(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    call_data: &[u8],
) -> InvokeResult<Vec<u8>> {
    // Perform the cross-contract call

    let mut return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());

    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        call_data,
        0, // TODO(geoff): invoke flags go here
        &mut return_len,
    ).map_err(InvokeError::Syscall);

    let is_ok = match result {
        Ok(()) => true,
        Err(InvokeError::Syscall(syserr)) if syserr.is_aborted() => false,
        Err(e) => {
            return Err(e)
        }
    };

    // Ok or abort: get the return buffer
    let mut return_buf = vec![0; return_len as usize];
    let mut written: RuntimeMemoryLength = 0;
    sys::raw_GetReturnedBuffer(return_buf.as_mut_slice(), &mut written)
        .unwrap_or_else(|_| contract_panic!("couldn't get return buffer"));
    if written != return_len {
        contract_panic!("unexpected return buffer length");
    }

    if is_ok {
        Ok(return_buf)
    } else {
        Err(InvokeError::Aborted(return_buf))
    }
}

// Invoke a client contract where we know the error type (which
// is produced on abort)
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
    match invoke(address, entrypoint, gas_limit, args) {
        Ok(ret) => Ok(ret),
        Err(err) => {
            match err {
                InvokeError::Aborted(buf) => {
                    match decode(&buf) {
                        Ok(val) => Err(ClientError::Aborted(val)),
                        Err(_) => Err(ClientError::AbortDecodeFailed),
                    }
                }
                _ => Err(ClientError::Invoke(err))
            }
        }
    }
}
