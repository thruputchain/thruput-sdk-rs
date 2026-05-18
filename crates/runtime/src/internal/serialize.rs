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

const ENCODE_MAX: usize = {
    let event_max = groundhog_xdr::event::kMaxEventLength as usize;
    if event_max > 500 { event_max } else { 500 }
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
