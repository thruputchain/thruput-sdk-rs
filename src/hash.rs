//! Hash trait

use crate::sysresult::Result;

pub type Hash256 = crate::crypto::SHA256Hash;

pub trait Hash {
    fn try_hash(&self) -> Result<Hash256>;

    /// Panics where `try_hash` returns `Err`
    fn hash(&self) -> Hash256 {
        self.try_hash()
            .unwrap_or_else(|e| panic!("Hash syscall failed: {}", e))
    }
}

#[cfg(feature = "alloc")]
const _: () = {
    use crate::sysresult::{Error, Result};

    impl<T> Hash for T
    where
        T: xdrs::WriteXDR,
    {
        fn try_hash(&self) -> Result<Hash256> {
            let buf = xdrs::serialize_txs::encode(&self)
                .map_err(Error::serialization)?;

            let hash = crate::crypto::sha_256(&buf)?;
            Ok(hash)
        }
    }
};

#[cfg(not(feature = "alloc"))]
const _: () = {
    use super::*;
    use crate::sysresult::{Error, Result};

    // For hashing `T: WriteXDR` without "alloc", it would be nice to have a chunked encoding
    // interface that we could feed into the hasher.  For now, we use a static buffer to hold the
    // entire encoding before hashing.

    const HASH_INPUT_MAX: usize = 500;

    static_mu_buf! {
        fn get_hash_buf<HASH_INPUT_MAX>(size_hint: Option<usize>) -> HashBuf;
    }

    impl<T> Hash for T
    where
        T: xdrs::WriteXDR,
    {
        fn try_hash(&self) -> Result<Hash256> {
            let mut hash_buf = get_hash_buf(None);

            let written = xdrs::serialize_txs::encode_into(
                &self,
                hash_buf.spare_capacity_mut(),
            )
            .map_err(|e| Error::serialization(e))?;

            // SAFETY: As required, this many bytes have been initialized.
            unsafe {
                hash_buf.set_len(written);
            }

            let hash = crate::crypto::sha_256(hash_buf.as_ref())?;
            Ok(hash)
        }
    }
};
