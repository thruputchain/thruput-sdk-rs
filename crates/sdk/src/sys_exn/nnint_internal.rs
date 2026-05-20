//! Wrappers for the raw nnint syscalls
//!
//! A storage location can be in the following states:
//! - Never before set
//! - Set to an item-type other than nnint
//! - Set to a nnint
//!
//! In the first two cases, we say that the location "does not contain a
//! nnint".
//!
//! During a transaction, pending "set" and "add" values are maintained for the
//! cell when it contains a nnint, notionally with types `Option<i128>` and
//! `i128`, respectively.
//!
//! Calling `set_add(s_opt, a)` replaces these pending values; this always
//! succeeds unless `s_opt.is_some_and(|s| s + a < 0)`.
//!
//! Calling `get_values` (without forcing) returns the pending values without
//! consulting the database.  It provides a "set" value only if a set operation
//! has been used in this transaction.
//!
//! The function `get_values_force` returns the pending "set" value if
//! available, or else loads the snapshot value from the database. This aborts
//! if the location does not contain a non-negative-int.

use groundhog_xdr::storage_item::StorageDeltaFlags;

use crate::exn::syscall_throw;
use crate::sys::{self, SysResult, SyscallNo};
use crate::*;

/// Replace the "set" and "add" values.
///
/// This discards any values previously present.
///
/// This aborts if `set_value.is_some_and(|s| s + a < 0)`.
///
pub fn set_add(
    key: &StorageKey,
    set_value: Option<i128>,
    add_value: i128,
    flags: StorageDeltaFlags,
) {
    sys::raw_NonnegativeIntSetAdd(
        key,
        set_value.as_ref(),
        &add_value,
        flags.0,
    )
    .unwrap_or_else(|e| syscall_throw(SyscallNo::kNonnegativeIntSetAdd, e));
}

/// Get the pending "set" and "add" values.
///
/// If the storage location has not yet been loaded, this succeeds no matter
/// what is at the location (including nothing or a value of another storage
/// type).
///
pub fn get_values(key: &StorageKey) -> (Option<i128>, i128) {
    _get_values(key, false)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kNonnegativeIntGet, e))
}

/// Get the pending "set" and "add" values, loading if necessary.
///
/// This behaves like `get_values`, except if the pending "set" value is
/// `None`, we instead load and return the last-committed value.
///
/// This aborts if the location does not contain a nnint.
///
pub fn get_values_force(key: &StorageKey) -> (i128, i128) {
    match _get_values(key, true) {
        Ok((s_opt, a)) => {
            // The "set" value is guaranteed to be present now because we
            // specified "force"
            let s = s_opt.unwrap();

            (s, a)
        }
        Err(e) => syscall_throw(SyscallNo::kNonnegativeIntGet, e),
    }
}

/// Optionally get the pending "set" and "add" values, loading if necessary.
///
/// This behaves like `get_values_force`, except it returns `None` if the
/// storage location has never been written.  (If a value of another storage
/// type is there, it aborts.)
///
pub fn try_get_values_force(key: &StorageKey) -> Option<(i128, i128)> {
    match _get_values(key, true) {
        Ok((s_opt, a)) => {
            // The "set" value is guaranteed to be present now because we
            // specified "force"
            let s = s_opt.unwrap();

            Some((s, a))
        }
        Err(e) if e.is_storage_key_not_exist() => None,
        Err(e) => syscall_throw(SyscallNo::kNonnegativeIntGet, e),
    }
}

// Returns an error if `force_load_set` is true and the location does not
// contain a non-negative integer.
//
// Otherwise, this always succeeds no matter what is at the location.
//
fn _get_values(
    key: &StorageKey,
    force_load_set: bool,
) -> SysResult<(Option<i128>, i128)> {
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
