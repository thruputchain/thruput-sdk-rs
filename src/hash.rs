//! Hash trait

use crate::crypto::{sha_256, SHA256Hash};
use crate::sysresult::{Result, Error};

pub type Hash256 = SHA256Hash;

pub trait Hash {
    fn try_hash(&self) -> Result<Hash256>;

    /// Panics where `try_hash` returns `Err`
    fn hash(&self) -> Hash256 {
        self.try_hash()
            .unwrap_or_else(|e| panic!("Hash syscall failed: {}", e))
    }
}

#[cfg(feature = "alloc")]
impl<T> Hash for T
where
    T: xdrs::WriteXDR,
{
    fn try_hash(&self) -> Result<Hash256> {
        let buf = xdrs::serialize_txs::encode(&self).map_err(|e|
            Error::serialization(e))?;

        let hash = sha_256(&buf)?;
        Ok(hash)
    }
}
