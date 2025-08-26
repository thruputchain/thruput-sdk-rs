use super::error::Result;
use super::keypath::*;
use crate::bytestring::*;
use crate::types::StorageKey;
use xdrs::serialize_cxs::encode as encode_xdr;
use xdrs::WriteXDR;

pub trait Store {
    fn store(&self, key: &StorageKey) -> Result<()>;

    fn store_keypath(&self, keypath: &Keypath) -> Result<()> {
        let key = keypath.generate_key();
        self.store(&key)
    }
}

impl<T> Store for T
where
    T: WriteXDR,
{
    fn store(&self, key: &StorageKey) -> Result<()> {
        // Load any previous value and drop it? XXX
        if let Some(prev) = bytestring_try_get(key)? {
            drop(prev);
        }

        bytestring_set(key, &encode_xdr(self)?)?;
        Ok(())
    }
}

/*
pub fn store<T: Store>(key: &StorageKey, val: &T) -> Result<()> {
    val.store(key)
}
*/
