//! System function: Delete

use crate::storage_delta_flags::*;
use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::StorageKey;

/// Delete anything stored at the given key
///
/// Succeeds no matter what type of item is stored there, or if none
/// is. After calling this function for a particular key, an item of
/// any type may be stored there.
pub fn delete_shared(key: &StorageKey) -> Result<()> {
    let flags = kStorageNone;

    sys::raw_DeleteObject(key, flags as i32)
}

/// Like [`delete`], but ... ?
pub fn delete_exclusive(key: &StorageKey) -> Result<()> {
    let flags = kStorageExclusiveLock;

    sys::raw_DeleteObject(key, flags as i32)
}

pub fn delete(key: &StorageKey, exclusive: bool) -> Result<()> {
    if exclusive {
        delete_exclusive(key)
    } else {
        delete_shared(key)
    }
}
