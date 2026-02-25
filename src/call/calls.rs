//! Making contract calls

use super::call_result::*;
use super::contract_result::*;
use super::internal::contract_call_encode;
use crate::abort;
use crate::contract::ContractEntrypointID;
use crate::exn::{CallingError, Exn, FaultString, LibraryExn, library_abort};
use crate::sys::{self, SysErr};
use crate::types::{ContractAddress, RuntimeMemoryLength};
use crate::xdr::*;

pub fn call<ARGS, RET, EXN>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> RET
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
    EXN: Exn + 'static,
{
    try_call(address, entrypoint, gas_limit, args).unwrap_or_else(
        |e: CallError<EXN>|
            // We're not going to capture the structure of the called
            // contract's error here (for that, use `try_call`).  Instead,
            // serialize the error and abort.
            abort(&LibraryExn::Called(FaultString::from_display(e))),
    )
}

pub fn try_call<ARGS, RET, EXN>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> CallResult<RET, EXN>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
    EXN: Exn,
{
    // Encode the args
    let args_buf = contract_call_encode(args)
        // Encoding failed: abort
        .unwrap_or_else(|e| {
            library_abort(CallingError::EncodingArgs(
                FaultString::from_display(e),
            ))
        });

    // Invoke the method
    let contract_result =
        contract_call(address, entrypoint, gas_limit, &args_buf)
            .unwrap_or_else(|syserr|
                // Invoke syscall failed: abort
                library_abort(CallingError::Syscall(syserr)));

    // Decode result
    into_call_result(contract_result)
}

#[inline(never)]
#[allow(clippy::result_large_err)]
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
) -> Result<ContractResult, SysErr>
where
    A: AsRef<[u8]> + ?Sized,
{
    // Perform the cross-contract call
    let mut return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        args.as_ref(),
        0, // TODO(geoff): invoke flags go here
        &mut return_len,
    );

    let is_ok = match result {
        Ok(()) => true,
        Err(syserr) if syserr.is_aborted() => false,
        Err(syserr) => {
            // Neither ok nor aborted
            return Err(syserr);
        }
    };

    // Ok or abort: get the return data from the VM ...

    let mut return_buf = ReturnBuf::new();
    fill_return_buf(return_len, &mut return_buf);

    let contract_result = if is_ok {
        Ok(return_buf)
    } else {
        Err(ContractError::Abort(return_buf))
    };

    Ok(contract_result)
}

#[inline]
fn fill_return_buf(return_len: u32, return_buf: &mut ReturnBuf) {
    let mut written_mu: core::mem::MaybeUninit<RuntimeMemoryLength> =
        core::mem::MaybeUninit::uninit();

    sys::uninit_GetReturnedBuffer(
        return_buf.spare_capacity_mut(),
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

    // Safety: The VM reports above that this many bytes were written
    unsafe {
        return_buf.set_len(written as usize);
    }
}
