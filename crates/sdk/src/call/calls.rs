//! Making contract calls

use super::contract_result::*;
use super::encode::*;
use super::raw_call_error::*;
use crate::contract::ContractEntrypointID;
use crate::exn::FaultString;
use crate::types::transaction::InvocationFrameStatusCode;
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
    flags: crate::invoke_config::InvokeFlagSet,
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
        flags,
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
    flags: crate::invoke_config::InvokeFlagSet,
) -> Result<ContractResult, RawCallError>
where
    A: AsRef<[u8]> + ?Sized,
{
    // Perform the cross-contract call
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
    let auth_bytes = auth_handles_as_bytes(auth_token_ids.as_slice());
    let inputs = sys::CopyableInvokeInputs {
        entrypoint_id: entrypoint,
        gas_limit,
        flags,
    };
    let mut outputs = sys::CopyableInvokeOutputs {
        return_buffer_len: 0,
        returned_auth_token_len: 0,
        // Placeholder; overwritten by the syscall whenever a child frame runs
        // (i.e. when `raw_Invoke` returns `Ok`). Never read on the error path.
        frame_status: InvocationFrameStatusCode::Fault,
    };
    let result = sys::raw_Invoke(
        address,
        args.as_ref(),
        auth_bytes,
        &inputs,
        &mut outputs,
    );

    let is_ok = match result {
        // A child frame ran; its outcome is reported in `frame_status`.
        Ok(()) => {
            let status = outputs.frame_status;
            if status == InvocationFrameStatusCode::Return {
                true
            } else if status == InvocationFrameStatusCode::Abort {
                false
            } else {
                // Fault (trap / out of gas) or an unexpected status code.
                return Err(RawCallError::Fault(FaultString::from_display(
                    format_args!("child frame faulted: {status:?}"),
                )));
            }
        }
        // The call was rejected before any child frame ran.
        Err(syserr) if syserr.is_invalid_input() => {
            return Err(RawCallError::Invalid);
        }
        Err(syserr) => {
            return Err(RawCallError::Fault(FaultString::from_display(
                format_args!("{syserr:?}"),
            )));
        }
    };

    // Ok or exception: get the return data from the VM ...

    let return_len = outputs.return_buffer_len;
    let auth_token_id_list_return_len = outputs.returned_auth_token_len;

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
    .map_err(|syserr| FaultString::from_display(format_args!("{syserr:?}")))?;

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
