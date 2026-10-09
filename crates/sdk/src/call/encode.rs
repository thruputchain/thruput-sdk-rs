use crate::call::{CallFault, RawCallError};
use crate::exn::FaultString;
use crate::{ReceiveAuthHandles, receive_auth_handles_from_iter, xdr::*};

pub fn contract_call_decode<'b, T>(
    bytes: &'b [u8],
    handles: impl IntoIterator<Item = u32>,
) -> Result<T, CallFault>
where
    T: ReadXDR<'b> + ReceiveAuthHandles,
{
    let mut args = cxs::decode(bytes).map_err(|e| {
        RawCallError::UnparseableReturn(FaultString::from_display(e))
    })?;

    receive_auth_handles_from_iter(&mut args, handles).map_err(|e| {
        RawCallError::UnparseableAuthHandles(FaultString::from_display(e))
    })?;
    Ok(args)
}
pub fn contract_call_decode_exn_prefix<'b, T>(
    bytes: &'b [u8],
) -> Result<T, ReadError>
where
    T: ReadXDR<'b>,
{
    let mut reader = cxs::buf_reader::BufReader::new(bytes);
    ReadXDR::read_xdr(&mut reader)
}

use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};

pub fn collect_auth_handle_bytes<T: CollectAuthHandles + ?Sized>(
    args: &T,
) -> Result<AuthHandleBuf, FaultString> {
    let mut handles = AuthHandleBuf::new();
    args.collect_auth_handles(&mut handles)?;
    Ok(handles)
}

pub fn auth_handles_as_mut_bytes(
    handles: &mut AuthHandleBuf,
) -> &mut [core::mem::MaybeUninit<u8>] {
    let slice = handles.as_mut_slice();
    // Safety: &mut [u32] → &mut [MaybeUninit<u8>] is always valid; u8 has alignment 1.
    unsafe {
        core::slice::from_raw_parts_mut(
            slice.as_mut_ptr() as *mut core::mem::MaybeUninit<u8>,
            core::mem::size_of_val(slice),
        )
    }
}

pub fn auth_handles_as_bytes(handles: &[u32]) -> &[u8] {
    // Safety: &[u32] → &[u8] is always valid; u8 has alignment 1.
    unsafe {
        core::slice::from_raw_parts(
            handles.as_ptr() as *const u8,
            core::mem::size_of_val(handles),
        )
    }
}
