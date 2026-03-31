use super::keypath::*;
use crate::sys_exn::bytestring::*;
use crate::types::StorageKey;
use alloc::vec::Vec;
use xdrs::WriteXDR;
use xdrs::cxs::encode;

pub trait Store {
    fn store(&self, key: &StorageKey);
    fn store_exclusive(&self, key: &StorageKey);

    fn store_keypath(&self, keypath: &Keypath) {
        let key = keypath.generate_key();
        self.store(&key);
    }
}

impl<T> Store for T
where
    T: WriteXDR,
{
    fn store(&self, key: &StorageKey) {
        let buf = store_pre(self, key);

        bytestring_set(key, buf.as_slice());
    }

    fn store_exclusive(&self, key: &StorageKey) {
        let buf = store_pre(self, key);

        bytestring_set_exclusive(key, buf.as_slice());
    }
}

// Common part of store/store_exclusive
fn store_pre<T: WriteXDR>(value: &T, key: &StorageKey) -> Vec<u8> {
    if core::mem::needs_drop::<T>() {
        // Load any previous value and drop it
        if let Some(prev) = bytestring_try_get(key) {
            drop(prev);
        }
    }

    encode(value).unwrap() // XX
}

pub fn store<T: Store>(key: &StorageKey, val: &T) {
    val.store(key)
}

/// Create a default storage value via [`FromKeypath`],
/// pass it to the given function for initialization, and
/// store it at the given keypath.
pub fn initialize<T, F>(keypath: &Keypath, mut init: F)
where
    T: Store + FromKeypath,
    F: FnMut(&mut T),
{
    let mut val = FromKeypath::from_keypath(keypath);
    init(&mut val);
    val.store_keypath(keypath);
}

/// Create a default storage value via [`FromKeypath`],
/// pass it to the given function for initialization, and
/// store it at the root key.
pub fn initialize_root<T, F>(init: F)
where
    T: Store + FromKeypath,
    F: FnMut(&mut T),
{
    let keypath = Keypath::root();
    initialize(&keypath, init)
}
