//! Hash trait

use crate::crypto::sha_256;
use crate::sysresult::Result;

pub type Hash256 = crate::crypto::SHA256Hash;

pub trait Hash {
    fn try_hash(&self) -> Result<Hash256> {
        Ok(self.hash())
    }

    /// Should panic where `try_hash` gives an error.
    /// If hashing may fail for some reason other than the hash syscall failing (which case is best
    /// to handle with a panic), implement `try_hash` instead.
    fn hash(&self) -> Hash256 {
        self.try_hash()
            .unwrap_or_else(|e| panic!("Hash failed: {}", e))
    }
}

#[cfg(feature = "dynamic_bufs")]
const _: () = {
    use crate::sysresult::{Error, Result};

    impl<T> Hash for T
    where
        T: xdrs::WriteXDR,
    {
        fn try_hash(&self) -> Result<Hash256> {
            let buf = xdrs::serialize_txs::encode(&self)
                .map_err(Error::serialization)?;

            let hash = sha_256(&buf)?;
            Ok(hash)
        }
    }
};

#[cfg(not(feature = "dynamic_bufs"))]
const _: () = {
    use super::*;
    use crate::sysresult::{Error, Result};

    // For hashing `T: WriteXDR` without "alloc", it would be nice to have a chunked encoding
    // interface that we could feed into the hasher.  For now, we use a static buffer to hold the
    // entire encoding before hashing.

    impl<T> Hash for T
    where
        T: xdrs::WriteXDR,
    {
        fn try_hash(&self) -> Result<Hash256> {
            let buf = crate::internal::encode_xdr(self)
                .map_err(Error::serialization)?;

            let hash = sha_256(buf.as_ref())?;
            Ok(hash)
        }
    }
};
