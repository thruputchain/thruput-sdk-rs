#![allow(non_snake_case)]

use crate::result::SysResult as Result;
use crate::sys;
use crate::types::StorageKeyContractData;

/// Delete anything stored at the given key
///
/// Succeeds no matter what type of item is stored there, or if none
/// is. After calling this function for a particular key, an item of
/// any type may be stored there.
pub fn delete(key: &StorageKeyContractData, exclusive: bool) -> Result<()> {
    let flags = if exclusive {
        groundhog_xdr::storage_delta_flags::kStorageExclusiveLock as i32
    } else {
        groundhog_xdr::storage_delta_flags::kStorageNone as i32
    };
    sys::raw_DeleteObject(key, flags as i32)
}
