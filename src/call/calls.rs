//! Making contract calls

use super::contract_result::*;
use super::encode::*;
use super::raw_call_error::*;
use crate::contract::ContractEntrypointID;
use crate::exn::FaultString;
use crate::types::{ContractAddress, RuntimeMemoryLength};
use crate::xdr::*;
use crate::{AuthHandleBuf, sys};

// Used by the contract macro in method wrappers
#[cfg_attr(feature = "debugging", track_caller)]
pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> Result<RET, RawCallError>
where
    ARGS: WriteXDR + crate::CollectAuthHandles,
    RET: ReadXDROwned + crate::ReceiveAuthHandles,
{
    let args_buf = match contract_call_encode(args) {
        Ok(buf) => buf,
        Err(e) => {
            return Err(RawCallError::EncodingArgs(
                FaultString::from_display(e),
            ));
        }
    };

    let auth_handles = match collect_auth_handle_bytes(args) {
        Ok(handles) => handles,
        Err(e) => {
            return Err(RawCallError::EncodingArgs(
                FaultString::from_display(e),
            ));
        }
    };

    let contract_result = contract_call(
        address,
        entrypoint,
        gas_limit,
        &args_buf,
        &auth_handles,
    )?;

    into_raw_call_result(contract_result)
}

#[inline(never)]
#[allow(clippy::result_large_err)]
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
    auth_token_ids: &crate::collect_auth_handles::AuthHandleBuf,
) -> Result<ContractResult, RawCallError>
where
    A: AsRef<[u8]> + ?Sized,
{
    // Perform the cross-contract call
    let mut return_len = 0;
    let mut auth_token_id_list_return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
    let auth_bytes = auth_handles_as_bytes(auth_token_ids.as_slice());
    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        args.as_ref(),
        0, // TODO(geoff): invoke flags go here
        auth_bytes,
        &mut return_len,
        &mut auth_token_id_list_return_len,
    );

    let is_ok = match result {
        Ok(()) => true,
        Err(syserr) if syserr.is_aborted() => false,
        Err(syserr) if syserr.is_invalid_input() => {
            return Err(RawCallError::Invalid);
        }
        Err(syserr) => {
            return Err(RawCallError::Fault(FaultString::from_display(
                syserr,
            )));
        }
    };

    // Ok or exception: get the return data from the VM ...

    let mut return_buf = ReturnBuf::new();
    let mut auth_token_id_list_buf = AuthHandleBuf::new();
    let auth_handle_count =
        auth_token_id_list_return_len as usize / core::mem::size_of::<u32>();
    #[cfg(not(feature = "alloc"))]
    auth_token_id_list_buf
        .resize(auth_handle_count, 0)
        .map_err(|_| {
            RawCallError::UnparseableAuthHandles(FaultString::from_display(
                "auth handle buffer capacity exceeded",
            ))
        })?;
    #[cfg(feature = "alloc")]
    auth_token_id_list_buf.resize(auth_handle_count, 0);
    if let Err(fault) = fill_return_buf(
        return_len,
        auth_token_id_list_return_len,
        &mut return_buf,
        &mut auth_token_id_list_buf,
    ) {
        return Err(if is_ok {
            RawCallError::UnparseableReturn(fault)
        } else {
            RawCallError::UnparseableAbort(fault)
        });
    }

    let contract_result = if is_ok {
        Ok((return_buf, auth_token_id_list_buf))
    } else {
        Err(ContractError::Abort(return_buf))
    };

    Ok(contract_result)
}

#[inline]
fn fill_return_buf(
    return_len: u32,
    auth_token_id_list_return_len: u32,
    return_buf: &mut ReturnBuf,
    auth_token_id_list_buf: &mut AuthHandleBuf,
) -> Result<(), FaultString> {
    let mut written_mu: core::mem::MaybeUninit<RuntimeMemoryLength> =
        core::mem::MaybeUninit::uninit();
    let mut auth_token_id_list_written_mu: core::mem::MaybeUninit<
        RuntimeMemoryLength,
    > = core::mem::MaybeUninit::uninit();

    let auth_token_id_list_bytes =
        auth_handles_as_mut_bytes(auth_token_id_list_buf);

    sys::uninit_GetReturnedBuffer(
        return_buf.spare_capacity_mut(),
        &mut written_mu,
        auth_token_id_list_bytes,
        &mut auth_token_id_list_written_mu,
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

    let written_auth_token_id_list =
        unsafe { auth_token_id_list_written_mu.assume_init() };
    if written_auth_token_id_list != auth_token_id_list_return_len {
        return Err(FaultString::from_display(format_args!(
            "unexpected auth token id list length from system call: \
                    written = {written_auth_token_id_list}, \
                    auth_token_id_list_return_len = {auth_token_id_list_return_len}"
        )));
    }

    Ok(())
}
