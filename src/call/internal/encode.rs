use crate::call::{ContractResult, ReturnBuf};
use crate::exn::{FaultString, HandlingError, library_throw};
use crate::xdr::*;

pub fn contract_call_decode<'b, T>(bytes: &'b [u8]) -> Result<T, ReadError>
where
    T: ReadXDR<'b>,
{
    cxs::decode(bytes)
}

/// Like `contract_call_decode`, but tolerates trailing bytes after the
/// decoded value.  Used for decoding exception payloads which may carry
/// additional debugging information (location, backtrace) appended by
/// `ContractExn` when the `debugging` feature is active.
pub fn contract_call_decode_prefix<'b, T>(
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

pub fn encode_success<T>(v: &T) -> ReturnBuf
where
    T: WriteXDR + ?Sized,
{
    contract_call_encode(v).unwrap_or_else(|e| {
        library_throw(HandlingError::EncodingSuccess(
            FaultString::from_display(e),
        ))
    })
}

pub fn contract_result_encode(r: &ContractResult) -> ReturnBuf {
    contract_call_encode(r).unwrap_or_else(|e| {
        if r.is_ok() {
            library_throw(HandlingError::EncodingSuccess(
                FaultString::from_display(e),
            ))
        } else {
            library_throw(HandlingError::EncodingException(
                FaultString::from_display(e),
            ))
        }
    })
}

use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};

pub fn collect_auth_handle_bytes<T: CollectAuthHandles>(
    args: &T,
) -> AuthHandleBuf {
    let mut handles = AuthHandleBuf::new();
    args.collect_auth_handles(&mut handles);
    handles
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
