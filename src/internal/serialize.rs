//! Serialization
//!
//! This provides `decode_xdr` directly from `xdrs`.
//!
//! If feature `dynamic_bufs` (and thus `alloc`) is enabled, `encode_xdr` is
//! provided directly from `xdrs`; otherwise a single, static buffer is used
//! for all encoding (by `contract_return` and `contract_abort`).

pub use xdrs::{
    WriteXDR,
    WriteResult,
    write_xdr,
    ReadXDR,
    ReadResult,
    read_xdr,
    ReadXDROwned,
};

pub use xdrs::serialize_txs::decode as decode_xdr;

#[cfg(feature = "dynamic_bufs")]
mod implementation {
    pub use xdrs::serialize_txs::encode as encode_xdr;
}

#[cfg(not(feature = "dynamic_bufs"))]
mod implementation {
    use super::*;
    use crate::static_mu_buf;

    const ENCODE_MAX: usize = 500;
    static_mu_buf! {
        fn get_encode_buf<ENCODE_MAX>(size_hint: Option<usize>) -> EncodeBuf;
    }

    /// XDR-encode a value into a static buffer.
    ///
    /// Panics: Calling this function again before dropping the previously-returned buffer will
    /// result in a panic.
    pub fn encode_xdr<'a, T>(x: &'a T) -> WriteResult<EncodeBuf>
    where
        T: WriteXDR + ?Sized
    {
        let mut buf = get_encode_buf(None);
        let written = xdrs::serialize_txs::encode_into(x, buf.spare_capacity_mut())?;
        unsafe { buf.set_len(written); }

        Ok(buf)
    }
}

pub use implementation::*;
