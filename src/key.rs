//! Construct `StorageKey` values.
//!
//! This module provides one, optional method of constructing storage keys.

use crate::internal::MUBuf;
use crate::*;

pub trait IntoKey: WriteXDR {
    /// Create a `StorageKey` that is highly likely to be unique to this
    /// object.
    ///
    /// If the `CXS` encoding of the value fits into a `StorageKey`, use that
    /// encoding directly. Otherwise, hash the `CXS` encoding.
    ///
    /// Invocations of this function must be annotated with a generic const
    /// parameter `MAX_ENCODED_SIZE`; a buffer of this size will be placed
    /// temporarily on the stack. If the encoding of the value exceeds the
    /// given size, this function will panic.
    ///
    fn as_key<const MAX_ENCODED_SIZE: usize>(&self) -> StorageKey {
        let buf: MUBuf<MAX_ENCODED_SIZE> = encode_xdr_stack(self);

        if buf.len() <= 32 {
            // Start with a known pattern (one that doesn't happen
            // to be a valid ContractAddress)
            let mut key = [0; 32];

            // Overwrite some prefix with the encoded data
            let key_contents = &mut key[..buf.len()];
            key_contents.copy_from_slice(buf.as_ref());

            return key.into();
        }

        sha_256(buf.as_ref())
            .unwrap_or_else(|e| panic!("SHA256 failed: {}", e))
            .into()
    }
}

impl<T> IntoKey for T where T: WriteXDR {}

fn encode_xdr_stack<const BUFSZ: usize, T>(
    x: &T,
) -> crate::internal::MUBuf<BUFSZ>
where
    T: WriteXDR + ?Sized,
{
    let mut buf = crate::internal::MUBuf::new();

    let len = xdrs::serialize_cxs::encode_into(x, buf.spare_capacity_mut())
        .unwrap_or_else(|e| panic!("encoding failed: {}", e));

    unsafe {
        buf.set_len(len);
    }

    buf
}
