//! Serialization
//!
//! # Decoding
//!
//! We provide `decode_xdr` directly from `xdrs`.
//!
//! # Encoding
//!
//! Allocate a buffer on the stack to receive the encoding.

pub use xdrs::cxs::decode as decode_xdr;
pub use xdrs::{
    ReadError, ReadResult, ReadXDR, ReadXDROwned, WriteError, WriteResult,
    WriteXDR, read_xdr, write_xdr,
};

use crate::internal::mu_buf::*;

/// Large enough to hold any value the SDK encodes into a fixed stack buffer:
/// events, storage bytestrings, and cross-contract return/abort payloads (plus
/// a 500-byte floor).
const ENCODE_MAX: usize = {
    let event_max = groundhog_xdr::event::kMaxEventLength as usize;
    let bytestring_max =
        groundhog_xdr::storage_item::kBytestringMaxLen as usize;
    let mut max = 500;
    if event_max > max {
        max = event_max;
    }
    if bytestring_max > max {
        max = bytestring_max;
    }
    max
};

pub type EncodeBuf = MUBuf<ENCODE_MAX>;

/// XDR-encode a value into a stack-allocated buffer
pub fn encode_xdr<T>(x: &T) -> WriteResult<EncodeBuf>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = MUBuf::new();

    let written = xdrs::cxs::encode_into(x, buf.spare_capacity_mut())?;

    unsafe {
        buf.set_len(written);
    }

    Ok(buf)
}
