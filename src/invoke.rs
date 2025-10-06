//! System functions: invoke, client_invoke

use crate::sys;
use crate::types::{ ContractAddress, RuntimeMemoryLength };
use crate::contract::ContractEntrypointID;
use crate::contract_panic;
use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};
use core::error::Error;
use crate::return_data::*;

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
    let return_data = invoke_inner(address, entrypoint, gas_limit, call_data.as_slice())?;

    // Deserialize the return data into a return value ...
    let return_val = decode(return_data.filled())
        .map_err(|_| InvokeError::DecodeReturnVal)?;

    Ok(return_val)
}

// At least this part won't be monomorphized multiple times
#[inline(never)]
fn invoke_inner(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    call_data: &[u8],
) -> InvokeResult<()> {
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
    );

    let is_ok = match result {
        Ok(()) => true,
        Err(syserr) if syserr.is_aborted() => false,
        Err(syserr) => return Err(InvokeError::Syscall(syserr)),
    };

    // Ok or abort: get the return data from the VM ...
 
    // SAFETY: This is the only call-site of static_return_data(), and this
    // function does not re-enter
    let mut return_data = unsafe { static_return_data() };
    let mut cursor = return_data.unfilled();

    // SAFETY: We promise not to uninitialize any bytes
    let cursor_mut = unsafe { cursor.as_mut() };

    let mut written: RuntimeMemoryLength = 0;

    sys::uninit_GetReturnedBuffer(cursor_mut, &mut written)
        .unwrap_or_else(|_| contract_panic!("couldn't get return data"));

    if written != return_len {
        contract_panic!("unexpected return data length; written = {written}, \
            return_len = {return_len}");
    }

    // SAFETY: The VM reports above that it has initialized this many bytes of the cursor
    unsafe {
        cursor.set_init(written as usize);
    }

    // We've filled this much of return_data
    cursor.advance(written as usize);

    if is_ok {
        Ok(())
    } else {
        Err(InvokeError::Aborted)
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
                InvokeError::Aborted => {
                    let return_data = unsafe { static_return_data() };
                    match decode(return_data.filled()) {
                        Ok(val) => Err(ClientError::Aborted(val)),
                        Err(_) => Err(ClientError::AbortDecodeFailed),
                    }
                }
                _ => Err(ClientError::Invoke(err))
            }
        }
    }
}
