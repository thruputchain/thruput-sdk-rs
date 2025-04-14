#![allow(non_snake_case)]

use crate::result::SysResult as Result;
use crate::sys;
use crate::types::StorageKey;

/// Delete anything stored at the given key
///
/// Succeeds no matter what type of item is stored there, or if none
/// is. After calling this function for a particular key, an item of
/// any type may be stored there.
pub fn delete(key: &StorageKey) -> Result<()> {
    sys::raw_DeleteObject(key)?;
    Ok(())
}
