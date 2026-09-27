use crate::sys::{self, SysErr};
use crate::sys_exn::flags::lock_after_write;
use crate::types::StorageKey;

/// Deletes the value at `key`. The key's storage flags stay as they are.
pub fn delete_shared(key: &StorageKey) -> Result<(), SysErr> {
    sys::raw_DeleteObject(key)
}

/// Deletes the value at `key`, then requests
/// [`ExclusiveLock`](crate::StorageDeltaFlags::ExclusiveLock) for it. If the
/// deletion fails, no lock is requested.
pub fn delete_exclusive(key: &StorageKey) -> Result<(), SysErr> {
    sys::raw_DeleteObject(key)?;
    lock_after_write(key, true);
    Ok(())
}
