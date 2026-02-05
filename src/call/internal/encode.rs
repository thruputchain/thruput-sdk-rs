use crate::xdr::*;
use crate::call::{ContractResult, ReturnBuf};
use crate::exn::{library_abort, HandlingError, FaultString};

pub fn contract_call_decode<'b, T>(bytes: &'b [u8]) -> Result<T, ReadError>
where
    T: ReadXDR<'b>
{
    cxs::decode(bytes)
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
    contract_call_encode(v)
        .unwrap_or_else(|e|
            library_abort(HandlingError::EncodingSuccess(FaultString::from_display(e))))
}

pub fn contract_result_encode(r: &ContractResult) -> ReturnBuf {
    contract_call_encode(r)
        .unwrap_or_else(|e|
            if r.is_ok() {
                library_abort(HandlingError::EncodingSuccess(FaultString::from_display(e)))
            } else {
                library_abort(HandlingError::EncodingAbort(FaultString::from_display(e)))
            })
}
