//! System functions: invoke, client_invoke

use crate::sys;
use crate::types::{ ContractAddress, RuntimeMemoryLength };
use crate::contract::ContractEntrypointID;
use crate::contract_panic;
use xdrs::{ReadXDROwned, WriteXDR};
use xdrs::serialize_txs::{encode, decode};
use core::error::Error;

pub use crate::invoke_error::*;

#[cfg(not(feature = "dynamic_bufs"))]
mod bufs {
    use crate::static_mu_buf;

    const MAX: usize = 500;

    pub const RETURN_BUF_MAX: Option<usize> = Some(MAX);

    static_mu_buf! {
        pub get_return_buffer<MAX>(size_hint: usize) -> ReturnData;
    }
}
#[cfg(feature = "dynamic_bufs")]
mod bufs {
    pub use alloc::vec::Vec;

    pub const RETURN_BUF_MAX: Option<usize> = None;

    pub type ReturnData = Vec<u8>;

    pub fn get_return_buffer(size: usize) -> ReturnData {
        Vec::with_capacity(size)
    }
}
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

    let return_val = decode(return_data.as_ref())
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
                match decode(return_data.as_ref()) {
                    Ok(val) => ClientError::Aborted(val),
                    Err(_) => ClientError::AbortDecodeFailed,
                }
            },
            RawError::Sys(syserr) =>
                ClientError::Invoke(InvokeError::Syscall(syserr)),
        }
    })?;

    let return_val = decode(return_data.as_ref())
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

    let mut return_data = get_return_buffer(return_len as usize);

    sys::uninit_GetReturnedBuffer(return_data.spare_capacity_mut(), &mut written)
        .unwrap_or_else(|_| contract_panic!("couldn't get return data"));

    if written != return_len {
        match RETURN_BUF_MAX {
            Some(max) if max < return_len as usize =>
                contract_panic!("overflowed the return-data buffer, \
                    which has a fixed length of {} bytes", max),
            _ =>
                contract_panic!("unexpected return-data length from system call: \
                    written = {written}, \
                    return_len = {return_len}"),
        }
    }

    // SAFETY: The VM reports above that this many bytes were written
    unsafe { return_data.set_len(written as usize); }

    if is_ok {
        Ok(return_data)
    } else {
        Err(RawError::Aborted(return_data))
    }
}
