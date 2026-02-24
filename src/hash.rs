//! Hash trait

use crate::crypto::sha256;

pub type Hash256 = crate::crypto::SHA256Hash;

pub trait Hash {
    fn hash(&self) -> Hash256;
}

// For now, we stack-allocate a buffer to hold the entire encoding before hashing.
// TODO: Use the incremental-hashing interface.

impl<T> Hash for T
where
    T: xdrs::WriteXDR,
{
    fn hash(&self) -> Hash256 {
        let buf = crate::internal::encode_xdr(self).unwrap(); // XXX

        sha256(buf.as_ref())
    }
}
