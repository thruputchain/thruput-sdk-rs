//! Wrappers for the raw nnint syscalls
//!
//! A storage location can be in the following states:
//! - never before set
//! - set to an item-type other than non-negative-int
//! - set to a value
//!
//! During a transaction, pending "set" and "add" values are maintained for the cell, notionally
//! with types `Option<i128>` and `i128`, respectively.
//!
//! Calling `set_add(s, a)` replaces these pending values, and calling `get_values` (without
//! forcing) returns them.
//!
//! The function `get_values_force` retrieves the pending "set" value if it
//! is present, or else loads from the database the value committed in the last block.

use groundhog_xdr::storage_item::StorageDeltaFlags;

use crate::sysresult::SysResult as Result;
use crate::*;

/// Replace the "set" and "add" values.  (This discards any values previously present.)
pub fn set_add(
    key: &StorageKey,
    set_value: Option<i128>,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(
        key,
        set_value.as_ref(),
        &add_value,
        StorageDeltaFlags::kStorageDeltaFlagNone as i32,
    )
}

/// Replace the "set" and "add" values.  (This discards any values previously present.)
///
/// If concurrent contract invocations set the value exclusively, only one such invocation can
/// succeed.
///
/// TODO(geoff): What about mixed exclusive/non-exclusive concurrent set?
///
pub fn set_add_exclusive(
    key: &StorageKey,
    set_value: Option<i128>,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(
        key,
        set_value.as_ref(),
        &add_value,
        StorageDeltaFlags::kStorageDeltaFlagExclusiveLock as i32,
    )
}

// Fails if `force_load_set` is true and the location does not contain a non-negative integer.
fn _get_values(
    key: &StorageKey,
    force_load_set: bool,
) -> Result<(Option<i128>, i128)> {
    let mut set_result: i128 = 0;
    let mut set_loaded: bool = false;
    let mut add_result: i128 = 0;
    sys::raw_NonnegativeIntGet(
        key,
        force_load_set,
        &mut set_result,
        &mut set_loaded,
        &mut add_result,
    )?;

    debug_assert!(if force_load_set { set_loaded } else { true });

    let set_result_opt = if set_loaded { Some(set_result) } else { None };

    Ok((set_result_opt, add_result))
}

// Get the pending "set" and "add" settings
pub fn get_values(key: &StorageKey) -> Result<(Option<i128>, i128)> {
    _get_values(key, false)
}

// Like `get_values`, except if the pending "set" value is `None`,
// we instead load and return the last-committed value.
pub fn get_values_force(key: &StorageKey) -> Result<(i128, i128)> {
    let (s_opt, a) = _get_values(key, true)?;

    // The "set" value is guaranteed to be present now because we specified "force"
    let s = s_opt.unwrap();

    Ok((s, a))
}

// Like `get_values_force`, except return `None` if the storage location has never been written or
// currently contains an item of another type.
pub fn try_get_values(key: &StorageKey) -> Result<Option<(i128, i128)>> {
    match get_values_force(key) {
        Ok(values) => Ok(Some(values)),
        Err(e) if e.is_storage_key_not_exist() => Ok(None),
        Err(e) => Err(e),
    }
}
