use super::error::Result;
use super::keypath::*;
use crate::bytestring::*;
use crate::serialize::{self, Serialize};
use crate::types::StorageKeyContractData;

pub trait Store {
    fn store(&self, key: &StorageKeyContractData) -> Result<()>;

    fn store_keypath(&self, keypath: &Keypath) -> Result<()> {
        let key = keypath.generate_key();
        self.store(&key)
    }
}

impl<T> Store for T
where
    T: Serialize,
{
    fn store(&self, key: &StorageKeyContractData) -> Result<()> {
        // Load any previous value and drop it? XXX
        if let Some(prev) = bytestring_try_get(key)? {
            drop(prev);
        }

        let buf = serialize::to_bytes(self)?;
        bytestring_set(key, buf.as_slice())?;
        Ok(())
    }
}

pub fn store<T: Store>(key: &StorageKeyContractData, val: &T) -> Result<()> {
    val.store(key)
}

/// Create a default storage value via [`DefaultFromKeypath`],
/// pass it to the given function for initialization, and
/// store it at the given keypath.
pub fn initialize<T, F>(keypath: &Keypath, mut init: F) -> Result<()>
where
    T: Store + DefaultFromKeypath,
    F: FnMut(&mut T),
{
    let mut val = DefaultFromKeypath::default_from_keypath(keypath);
    init(&mut val);
    val.store_keypath(keypath)?;
    Ok(())
}

/// Create a default storage value via [`DefaultFromKeypath`],
/// pass it to the given function for initialization, and
/// store it at the root key.
pub fn initialize_root<T, F>(init: F) -> Result<()>
where
    T: Store + DefaultFromKeypath,
    F: FnMut(&mut T),
{
    let keypath = Keypath::root();
    initialize(&keypath, init)
}
