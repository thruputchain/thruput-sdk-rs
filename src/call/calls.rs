//! Making contract calls

use super::call_result::{RawCallError, into_raw_call_result};
use super::contract_result::*;
use super::internal::contract_call_encode;
use crate::contract::ContractEntrypointID;
use crate::exn::{CallingError, FaultString, library_throw};
use crate::sys;
use crate::types::{ContractAddress, RuntimeMemoryLength};
use crate::xdr::*;

pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> Result<RET, RawCallError>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
{
    let args_buf = contract_call_encode(args).unwrap_or_else(|e| {
        library_throw(CallingError::EncodingArgs(FaultString::from_display(e)))
    });

    let contract_result =
        contract_call(address, entrypoint, gas_limit, &args_buf)?;

    into_raw_call_result(contract_result)
}

/// Re-throw a raw exception to our caller unchanged.
pub fn re_throw(exn: &crate::exn::RawException) -> ! {
    exn.re_throw()
}

#[inline(never)]
#[allow(clippy::result_large_err)]
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
) -> Result<ContractResult, RawCallError>
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
            return Err(RawCallError::Fault(FaultString::from_display(
                syserr,
            )));
        }
    };

    // Ok or exception: get the return data from the VM ...

    let mut return_buf = ReturnBuf::new();
    if let Err(fault) = fill_return_buf(return_len, &mut return_buf) {
        return Err(if is_ok {
            RawCallError::UnparseableReturn(fault)
        } else {
            RawCallError::UnparseableAbort(fault)
        });
    }

    let contract_result = if is_ok {
        Ok(return_buf)
    } else {
        Err(ContractError::Abort(return_buf))
    };

    Ok(contract_result)
}

#[inline]
fn fill_return_buf(
    return_len: u32,
    return_buf: &mut ReturnBuf,
) -> Result<(), FaultString> {
    let mut written_mu: core::mem::MaybeUninit<RuntimeMemoryLength> =
        core::mem::MaybeUninit::uninit();

    sys::uninit_GetReturnedBuffer(
        return_buf.spare_capacity_mut(),
        &mut written_mu,
    )
    .map_err(FaultString::from_display)?;

    let written = unsafe { written_mu.assume_init() };

    if written != return_len {
        return Err(FaultString::from_display(format_args!(
            "unexpected return-data length from system call: \
                    written = {written}, \
                    return_len = {return_len}"
        )));
    }

    // Safety: The VM reports above that this many bytes were written
    unsafe {
        return_buf.set_len(written as usize);
    }

    Ok(())
}
