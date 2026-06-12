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

pub type EncodeBuf = heapless::Vec<u8, 500>;

/// CXS-encode a value into a stack-allocated buffer
///
pub fn contract_call_encode<T>(x: &T) -> WriteResult<EncodeBuf>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = EncodeBuf::new();

    let written = cxs::encode_into(x, buf.spare_capacity_mut())?;

    // Safety: The written bytes are valid.
    unsafe {
        buf.set_len(written);
    }

    Ok(buf)
}

/// CXS-encode a value for a terminal `Return`/abort: the instance dies
/// right after, so under `alloc` this takes the call scratch buffer (never
/// returning it) and grows up to the protocol's return-size cap
/// (`kMaxReturnBufferLen`); larger encodings error with `SpaceExhausted`
/// (a typed `EncodingException` at the call sites) rather than letting the
/// host reject the oversized `Return` as an opaque fault. Without `alloc`
/// it is a fixed 500-byte stack buffer.
pub(crate) fn encode_terminal<T>(x: &T) -> WriteResult<crate::call::Buf>
where
    T: WriteXDR + ?Sized,
{
    #[cfg(feature = "alloc")]
    {
        use crate::types::transaction::kMaxReturnBufferLen;

        let mut writer = cxs::buf_writer::BufWriter::from(
            super::calls::take_terminal_encode_buf(),
        );
        x.write_xdr(&mut writer)?;
        if writer.len() > kMaxReturnBufferLen as usize {
            return Err(WriteError::SpaceExhausted);
        }
        Ok(writer.into())
    }
    #[cfg(not(feature = "alloc"))]
    {
        contract_call_encode(x)
    }
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
