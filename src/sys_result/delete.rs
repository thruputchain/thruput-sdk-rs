use crate::sys::{self, SysErr};
use crate::types::StorageKey;
use groundhog_xdr::storage_item::*;

pub fn delete_shared(key: &StorageKey) -> Result<(), SysErr> {
    let flags = StorageDeltaFlags::kStorageDeltaFlagNone;

    sys::raw_DeleteObject(key, flags.0)
}

pub fn delete_exclusive(key: &StorageKey) -> Result<(), SysErr> {
    let flags = StorageDeltaFlags::kStorageDeltaFlagExclusiveLock;

    sys::raw_DeleteObject(key, flags.0)
}
