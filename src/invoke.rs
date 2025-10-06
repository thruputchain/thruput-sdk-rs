//! System functions: invoke, client_invoke

use crate::sys;
use crate::types::{ ContractAddress, RuntimeMemoryLength };
use crate::contract::ContractEntrypointID;
use crate::contract_panic;
use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};
use core::error::Error;

pub use crate::invoke_error::*;

// #[cfg(not(feature = "dynamic_bufs"))]
mod bufs {
    use crate::static_mu_buf;

    static_mu_buf! {
        pub get_static_return_data<200>() -> ReturnData
    }
}

/*
#[cfg(feature = "dynamic_bufs")]
mod bufs {
}
*/

use bufs::*;

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
    let return_data = invoke_raw(address, entrypoint, gas_limit, call_data.as_slice()).map_err(|e|
        match e {
            RawError::Sys(syserr) => InvokeError::Syscall(syserr),
            RawError::Aborted(_) => InvokeError::Aborted,
        })?;

    // SAFETY: We used `ReturnData::set_len` correctly in `invoke_raw`
    let return_data = unsafe { return_data.filled() };

    let return_val = decode(return_data)
        .map_err(|_| InvokeError::DecodeReturnVal)?;

    Ok(return_val)
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
    // Encode the args
    let call_data = encode(args).or(Err(InvokeError::EncodeArgs))?;

    let return_data = invoke_raw(address, entrypoint, gas_limit, call_data.as_slice()).map_err(|inner_err| {
        match inner_err {
            RawError::Aborted(return_data) => {
                // SAFETY: We use `ReturnData::set_len` appropriately
                let return_data = unsafe { return_data.filled() };

                match decode(return_data) {
                    Ok(val) => ClientError::Aborted(val),
                    Err(_) => ClientError::AbortDecodeFailed,
                }
            },
            RawError::Sys(syserr) =>
                ClientError::Invoke(InvokeError::Syscall(syserr)),
        }
    })?;

    // SAFETY: We used `ReturnData::set_len` correctly in `invoke_raw`
    let return_data = unsafe { return_data.filled() };

    let return_val = decode(return_data)
        .map_err(|_| InvokeError::DecodeReturnVal)?;

    Ok(return_val)
}

enum RawError {
    Sys(crate::SysErr),
    Aborted(ReturnData),
}

// At least this part won't be monomorphized multiple times
#[inline(never)]
fn invoke_raw(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    call_data: &[u8],
) -> core::result::Result<ReturnData, RawError> {
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
        Err(syserr) => return Err(RawError::Sys(syserr)),
    };

    // Ok or abort: get the return data from the VM ...
 
    let mut written: RuntimeMemoryLength = 0;
    let mut return_data = get_static_return_data();

    sys::uninit_GetReturnedBuffer(return_data.as_mut(), &mut written)
        .unwrap_or_else(|_| contract_panic!("couldn't get return data"));

    if written != return_len {
        contract_panic!("unexpected return data length; \
            written = {written}, \
            return_len = {return_len}");
    }

    return_data.set_len(written as usize);

    if is_ok {
        Ok(return_data)
    } else {
        Err(RawError::Aborted(return_data))
    }
}
