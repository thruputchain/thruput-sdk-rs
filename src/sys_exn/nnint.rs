//! System functions: non-negative integers
//!
//! At the start of a transaction, a storage location holds a non-negative-int
//! value if a previous transaction performed a "set" operation there, and the
//! most recent such operation placed a value of the non-negative-int storage
//! type. Otherwise, it does not contain a non-negative-int value.
//!
//! If the location does not contain a non-negative-int value, it may
//! nevertheless be set or incremented, after which it has a value in the
//! current transaction.  An increment operation behaves as if there had been a
//! previous value of zero.
//!
//! If the location _does_ have a non-negative-int value, it may be set,
//! incremented, or retrieved (by `non_negative_int_get`).
//!

use crate::exn::{nnint_overflow, nnint_underflow};
use crate::sys_exn::nnint_internal as nnint;
use crate::types::StorageKey;
use groundhog_xdr::storage_item::StorageDeltaFlags;

const NONE: StorageDeltaFlags = StorageDeltaFlags::None;
const EXCLUSIVE: StorageDeltaFlags =
    StorageDeltaFlags::ExclusiveLock;

/// Get the current value.
///
/// Throws an exception if the location does not contain a non-negative-int value.
///
pub fn non_negative_int_get(key: &StorageKey) -> i128 {
    let (s, a) = nnint::get_values_force(key);

    s.checked_add(a).unwrap_or_else(|| nnint_overflow())
}

/// Get the current value, or `None` if empty.
///
/// This behaves like `non_negative_int_get`, but returns `None` if the storage
/// location is empty.  It throws an exception if a value of the wrong type is there.
///
pub fn non_negative_int_try_get(key: &StorageKey) -> Option<i128> {
    nnint::try_get_values_force(key)
        .map(|(s, a)| s.checked_add(a).unwrap_or_else(|| nnint_overflow()))
}

/// Set the value.
///
/// (This clears the effects of any previous "increment" operations.)
///
/// This call always succeeds, but it puts constraints on the transaction,
/// described below.
///
/// A negative value is allowed, as this explicitly changes the absolute value.
/// If subsequent increments result in a negative sum again, that will not lead
/// to an exception.
///
/// If a non-negative value is set, and subsequent increments added to that
/// value would be negative at the end of the transaction, it will revert.
///
/// A set of concurrent transactions with the same explicitly-set value may
/// included in the same block.
///
pub fn non_negative_int_set(key: &StorageKey, set_value: i128) {
    nnint::set_add(key, Some(set_value), 0, NONE);
}

/// Set the value exclusively.
///
/// This behaves like [`non_negative_int_set`], with the additional constraint
/// that, if a concurrent exclusive-set operation is performed by another
/// contract invocation, only one such invocation can be included in the same
/// block.
///
pub fn non_negative_int_set_exclusive(key: &StorageKey, set_value: i128) {
    nnint::set_add(key, Some(set_value), 0, EXCLUSIVE);
}

/// Increment the value.
///
/// Throws an exception if the increments accumulated from previous calls would overflow.
///
/// If a "set" operation has been used previously in this transaction and the
/// value resulting from adding the accumulated increments `amount` is
/// negative, this throws an exception.
///
/// Otherwise, if the resulting value (the value at the end of the last
/// transaction plus the accumulated increments) is negative, this function
/// will not immediately throw.  But note the transaction constraints at
/// `non_negative_int_set`.
///
/// A set of concurrent transactions may be included in the same block if their
/// explicitly-set value is the same (see `non_negative_int_set`) and the sum
/// of that value and of all their increments is non-negative.
///
pub fn non_negative_int_inc(key: &StorageKey, amount: i128) {
    // Combine the existing "add" value with this additional increment
    let (s, a) = nnint::get_values(key);
    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    // Update the increment, keeping the "set" value (if any) as-is.
    nnint::set_add(key, s, add_value, NONE);
}

/// Increment the value.
///
/// This behaves similarly to `non_negative_int_inc`, except that it throws if
/// the sum of the explicitly-set value and the accumulated increments
/// would overflow, or if it is negative.
///
pub fn non_negative_int_strict_inc(key: &StorageKey, amount: i128) {
    let (s, a) = nnint::try_get_values_force(key).unwrap_or((0, 0));

    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    let total = s.checked_add(add_value).unwrap_or_else(|| nnint_overflow());

    if total < 0 {
        nnint_underflow();
    }

    nnint::set_add(key, Some(s), add_value, NONE);
}
