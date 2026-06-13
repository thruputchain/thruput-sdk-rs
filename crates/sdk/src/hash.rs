//! Hash trait

use crate::crypto::sha256;

pub type Hash256 = crate::crypto::SHA256Hash;

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
            crate::internal::encode_scratch(buf, self).unwrap();
            sha256(buf.as_slice())
        })
    }
}
