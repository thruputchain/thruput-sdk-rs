use crate::call::RawCallError;
use crate::exn::FaultString;
use crate::{ReceiveAuthHandles, receive_auth_handles_from_slice, xdr::*};

pub fn contract_call_decode<'b, T>(
    bytes: &'b [u8],
    handles: &[u32],
) -> Result<T, RawCallError>
where
    T: ReadXDR<'b> + ReceiveAuthHandles,
{
    let mut args = cxs::decode(bytes).map_err(|e| {
        RawCallError::UnparseableReturn(FaultString::from_display(e))
    })?;

    receive_auth_handles_from_slice(&mut args, handles).map_err(|e| {
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

/// CXS-encode a value for a terminal `Return`/abort. Every caller diverges
/// into `contract_abort` / `contract_return`, so the instance dies right
/// after and the buffer is used exactly once.
///
/// Encodings larger than the protocol's return-size cap
/// (`kMaxReturnBufferLen`) error with `SpaceExhausted` — surfaced as a typed
/// `EncodingException` at the call sites rather than letting the host reject
/// the oversized `Return` as an opaque fault. Without `alloc`, `Buf` is a
/// fixed 500-byte stack buffer, which caps it below that.
pub(crate) fn encode_terminal<T>(x: &T) -> WriteResult<crate::call::Buf>
where
    T: WriteXDR + ?Sized,
{
    use crate::types::transaction::kMaxReturnBufferLen;

    // The only build-dependent step: `Buf` is a heap `Vec` under `alloc` and
    // needs capacity reserved up front (the instance terminates right after,
    // so this lone allocation never recurs), whereas the no-alloc `Buf` is a
    // fixed inline buffer that already has its capacity.
    #[cfg(feature = "alloc")]
    let mut buf =
        crate::call::Buf::with_capacity(kMaxReturnBufferLen as usize);
    #[cfg(not(feature = "alloc"))]
    let mut buf = crate::call::Buf::new();

    let written = cxs::encode_into(x, buf.spare_capacity_mut())?;
    // Safety: `encode_into` initialized exactly `written` bytes.
    unsafe {
        buf.set_len(written);
    }
    // A heap `Buf` may over-reserve past the cap; enforce it explicitly.
    if buf.len() > kMaxReturnBufferLen as usize {
        return Err(WriteError::SpaceExhausted);
    }
    Ok(buf)
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
