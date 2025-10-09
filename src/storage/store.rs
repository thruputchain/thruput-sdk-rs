use super::error::{Error, Result};
use super::keypath::*;
use crate::bytestring::*;
use crate::types::StorageKey;
use xdrs::{WriteXDR};
use xdrs::serialize_cxs;

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
        // Load any previous value and drop it
        // TODO: Avoid where there is no Drop impl?
        if let Some(prev) = bytestring_try_get(key)? {
            drop(prev);
        }

        let buf = serialize_cxs::encode(self).map_err(|_|
            Error::Serialization)?;
        bytestring_set(key, buf.as_slice())?;
        Ok(())
    }
}

pub fn store<T: Store>(key: &StorageKey, val: &T) -> Result<()> {
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
