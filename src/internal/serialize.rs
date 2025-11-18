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

pub use xdrs::{
    ReadError, ReadResult, ReadXDR, ReadXDROwned, WriteError, WriteResult,
    WriteXDR, read_xdr, write_xdr,
};

pub use xdrs::serialize_txs::decode as decode_xdr;

/// XDR-encode a value into a sequentially-shared buffer.
///
/// Panics: Calling this function again before dropping the previously-returned buffer will
/// result in a panic (except in the testing environment, when `feature = "mock"`).
pub fn encode_xdr<T>(x: &T) -> WriteResult<EncodeBuf>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = get_encode_buf(None);
    let written =
        xdrs::serialize_txs::encode_into(x, buf.spare_capacity_mut())?;
    unsafe {
        buf.set_len(written);
    }
    Ok(buf)
}

#[cfg(feature = "dynamic_bufs")]
mod get_encode_buf_impl {
    // This strategy requires liballoc (via feature "alloc") and uses a dedicated,
    // statically-allocated `Vec<u8>` (that is, a fat-pointer to a dynamically-allocated buffer).
    // It is reallocated as necessary (only increasing in size) to accommodate the hinted size
    // requirement.
    //
    // A handle to this vec is returned on each call to `get_encode_buf`. Calling the function again
    // before dropping the previous handle will result in a panic.

    crate::static_vec! {
        pub fn get_encode_buf(size_hint: Option<usize>) -> EncodeBuf;
    }
}

#[cfg(not(feature = "dynamic_bufs"))]
mod get_encode_buf_impl {
    // This strategy uses a dedicated buffer with static storage.
    // It ignores the size hint.
    //
    // A handle to this buffer is returned on each call to `get_encode_buf`.
    // Calling the function again before dropping the previous handle will result in a panic.

    const ENCODE_MAX: usize = 500;

    crate::static_mu_buf! {
        pub fn get_encode_buf<ENCODE_MAX>(size_hint: Option<usize>) -> EncodeBuf;
    }
}

pub use get_encode_buf_impl::*;
