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

use crate::exn::{LibraryExn, syscall_exn, syscall_throw};
use crate::re_throw::OrReThrow;
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
    try_set_add(key, set_value, add_value, flags).or_re_throw()
}

/// Like [`set_add`], but report a syscall failure as `Err` instead of
/// aborting. Callers that pre-check the negativity constraint (so the
/// remaining error paths are invalid flags or host faults) use this to keep
/// the `try_` law.
#[inline]
pub(crate) fn try_set_add(
    key: &StorageKey,
    set_value: Option<i128>,
    add_value: i128,
    flags: StorageDeltaFlags,
) -> Result<(), LibraryExn> {
    sys::raw_NonnegativeIntSetAdd(key, set_value.as_ref(), &add_value, flags.0)
        .map_err(|e| syscall_exn(SyscallNo::kNonnegativeIntSetAdd, e))
}

/// Get the pending "set" and "add" values.
///
/// If the storage location has not yet been loaded, this succeeds no matter
/// what is at the location (including nothing or a value of another storage
/// type).
///
pub fn get_values(key: &StorageKey) -> (Option<i128>, i128) {
    try_get_values(key).or_re_throw()
}

/// Like [`get_values`], but report a syscall failure as `Err` instead of
/// aborting. Callers that pre-check the negativity constraint (so the
/// remaining error paths are invalid flags or host faults) use this to keep
/// the `try_` law.
#[inline]
pub(crate) fn try_get_values(key: &StorageKey) -> Result<(Option<i128>, i128), LibraryExn> {
    _get_values(key, false)
        .map_err(|e| syscall_exn(SyscallNo::kNonnegativeIntGet, e))
}

/// Get the pending "set" and "add" values, loading if necessary.
///
/// This behaves like `get_values`, except if the pending "set" value is
/// `None`, we instead load and return the last-committed value.
///
/// This aborts if the location does not contain a nnint.
///
pub fn get_values_force(key: &StorageKey) -> (i128, i128) {
    try_get_values_force(key).or_re_throw()
}

/// Get the pending "set" and "add" values, loading if necessary, and
/// reporting failure as `Err` instead of aborting (the `try_` law; see the
/// [`exn`](mod@crate::exn) module docs).
///
/// This behaves like `get_values_force`, except every failure — a
/// never-written location
/// (`Err(e)` with [`e.is_storage_missing()`](LibraryExn::is_storage_missing)),
/// a value of another storage type
/// ([`kSyscallErrorCodeInvalidSyscallInput`](crate::sys::SyscallErrorCode::kSyscallErrorCodeInvalidSyscallInput)),
/// or any other syscall error — is returned as the `LibraryExn::Syscall`
/// value the non-`try_` form would have thrown.
///
#[inline]
pub fn try_get_values_force(
    key: &StorageKey,
) -> Result<(i128, i128), LibraryExn> {
    match _get_values(key, true) {
        Ok((s_opt, a)) => {
            // The "set" value is guaranteed to be present now because we
            // specified "force"
            let s = s_opt.unwrap();

            Ok((s, a))
        }
        Err(e) => Err(syscall_exn(SyscallNo::kNonnegativeIntGet, e)),
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
