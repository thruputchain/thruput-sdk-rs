use crate::exn::{LibraryExn, syscall_exn};
use crate::re_throw::OrReThrow;
use crate::sys::{self, SyscallNo};
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Flags of the visible pending write at this key, including deletion.
/// Returns [`StorageDeltaFlags::None`] before any write; does not load the snapshot.
/// Read again after any call that could write this key.
pub fn get_storage_write_flags(key: &StorageKey) -> StorageDeltaFlags {
    try_get_storage_write_flags(key).or_re_throw()
}

/// Like [`get_storage_write_flags`], returning syscall errors to the caller.
pub fn try_get_storage_write_flags(
    key: &StorageKey,
) -> Result<StorageDeltaFlags, LibraryExn> {
    let mut flags = 0;
    sys::raw_GetStorageWriteFlags(key, &mut flags)
        .map_err(|e| syscall_exn(SyscallNo::kGetStorageWriteFlags, e))?;
    Ok(StorageDeltaFlags(flags))
}

pub(crate) fn preserve_exclusive_flag(
    key: &StorageKey,
    requested: StorageDeltaFlags,
) -> Result<StorageDeltaFlags, LibraryExn> {
    let prior = try_get_storage_write_flags(key)?;
    Ok(StorageDeltaFlags(
        requested.0 | (prior.0 & StorageDeltaFlags::ExclusiveLock.0),
    ))
}
