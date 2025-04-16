use crate::crypto::{sha_256, SHA256Hash};
use crate::storage::error::Result;

pub type Hash256 = SHA256Hash;

pub trait Hash {
    fn try_hash(&self) -> Result<Hash256>;

    /// Panics where `try_hash` returns `Err`
    fn hash(&self) -> Hash256 {
        self.try_hash()
            .unwrap_or_else(|e| panic!("Hash syscall failed: {}", e))
    }
}

use crate::serialize::{self, Serialize};
impl<T> Hash for T
where
    T: Serialize,
{
    fn try_hash(&self) -> Result<Hash256> {
        let buf = serialize::to_bytes(&self)?;
        let hash = sha_256(&buf)?;
        Ok(hash)
    }
}
