use crate::sys::{self, SysErr};
use crate::types::StorageKey;
use groundhog_xdr::storage_item::*;

use crate::sys_exn::flags::request_after_write;

/// Deletes the value at `key`. The key's storage flags stay as they are.
pub fn delete_shared(key: &StorageKey) -> Result<(), SysErr> {
    sys::raw_DeleteObject(key)
}

/// Deletes the value at `key`, then requests
/// [`ExclusiveLock`](StorageDeltaFlags::ExclusiveLock) for it.
pub fn delete_exclusive(key: &StorageKey) -> Result<(), SysErr> {
    delete_with_flags(key, StorageDeltaFlags::ExclusiveLock)
}

/// Deletes the value at `key`, then adds `flags` to the key's storage flags.
/// If the deletion fails, no flags are requested.
pub fn delete_with_flags(
    key: &StorageKey,
    flags: StorageDeltaFlags,
) -> Result<(), SysErr> {
    sys::raw_DeleteObject(key)?;
    request_after_write(key, flags);
    Ok(())
}
