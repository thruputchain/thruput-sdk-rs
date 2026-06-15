//! Hash trait

use crate::crypto::sha256;
use crate::{FaultString, HandlingError, library_throw};

pub type Hash256 = crate::crypto::SHA256Hash;

/// SHA-256 of a value's CXS encoding.
///
/// The blanket impl covers every [`WriteXDR`](xdrs::WriteXDR) type: it encodes
/// the value into the shared scratch buffer and hashes the bytes. Under the
/// `alloc` feature the scratch grows to fit, so there is no size limit. In the
/// no-alloc guest the scratch is a fixed [`SCRATCH_MAX`](crate::internal::SCRATCH_MAX)
/// (4 KiB) buffer; a value whose encoding exceeds it throws
/// [`HandlingError::EncodingException`] (a non-returning library exception),
/// matching how terminal returns and aborts handle the same overflow.
pub trait Hash {
    fn hash(&self) -> Hash256;
}

// We encode the whole value into the shared scratch buffer before hashing.
// TODO: Use the incremental-hashing interface.

impl<T> Hash for T
where
    T: xdrs::WriteXDR,
{
    fn hash(&self) -> Hash256 {
        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, self).unwrap_or_else(|e| {
                library_throw(HandlingError::EncodingException(
                    FaultString::via_display(&e),
                ))
            });
            sha256(buf.as_slice())
        })
    }
}
