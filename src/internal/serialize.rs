//! Serialization
//!
//! # Decoding
//!
//! We provide `decode_xdr` directly from `xdrs`.
//!
//! # Encoding
//!
//! A single buffer is re-used (currently by `log_event`).
//!
//! With `feature = dynamic_bufs`, the re-used buf is resized as necessary, so no
//! maximum size must be chosen.  Otherwise the storage is static and we choose
//! a maximum size.

pub use xdrs::serialize_txs::decode as decode_xdr;
pub use xdrs::{
    ReadError, ReadResult, ReadXDR, ReadXDROwned, WriteError, WriteResult,
    WriteXDR, read_xdr, write_xdr,
};

use crate::internal::mu_buf::*;

const ENCODE_MAX: usize = 500;

pub type EncodeBuf = MUBuf<ENCODE_MAX>;

/// XDR-encode a value into a stack-allocated buffer
pub fn encode_xdr<T>(x: &T) -> WriteResult<EncodeBuf>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = MUBuf::new();

    let written =
        xdrs::serialize_txs::encode_into(x, buf.spare_capacity_mut())?;

    unsafe {
        buf.set_len(written);
    }

    Ok(buf)
}
