use super::error::{Result, StorageError};
use super::keypath::*;
use crate::bytestring::*;
use crate::types::StorageKey;
use alloc::vec::Vec;
use xdrs::WriteXDR;
use xdrs::cxs::encode;

pub trait Store {
    fn store(&self, key: &StorageKey) -> Result<()>;
    fn store_exclusive(&self, key: &StorageKey) -> Result<()>;

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
        let buf = store_pre(self, key)?;

        bytestring_set(key, buf.as_slice())?;
        Ok(())
    }

    fn store_exclusive(&self, key: &StorageKey) -> Result<()> {
        let buf = store_pre(self, key)?;

        bytestring_set_exclusive(key, buf.as_slice())?;
        Ok(())
    }
}

// Common part of store/store_exclusive
fn store_pre<T: WriteXDR>(value: &T, key: &StorageKey) -> Result<Vec<u8>> {
    if core::mem::needs_drop::<T>() {
        // Load any previous value and drop it
        if let Some(prev) = bytestring_try_get(key)? {
            drop(prev);
        }
    }

    let buf = encode(value).map_err(|_| StorageError::Serialization)?;

    Ok(buf)
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
