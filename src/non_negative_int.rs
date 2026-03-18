//! System functions: non-negative integers
//!
//! At the start of a transaction, a storage location holds a non-negative-int
//! value if a previous transaction perfomed a "set" operation there, and the
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
use crate::internal::nnint;
use crate::types::StorageKey;

/// Get the current value.
///
/// Aborts if the location does not contain a non-negative-int value.
///
pub fn non_negative_int_get(key: &StorageKey) -> i128 {
    let (s, a) = nnint::get_values_force(key);

    s.checked_add(a).unwrap_or_else(|| nnint_overflow())
}

/// Get the current value, or `None` if empty.
///
/// This behaves like `non_negative_int_get`, but returns `None` if the storage
/// location is empty.  It aborts if a value of the wrong type is there.
///
pub fn non_negative_int_try_get(key: &StorageKey) -> Option<i128> {
    nnint::try_get_values_force(key)
        .map(|(s, a)| s.checked_add(a).unwrap_or_else(|| nnint_overflow()))
}

/// Set the value.
///
/// This call always succeeds, but it puts constraints on the transaction,
/// described below.
///
/// A negative value is allowed, as this explicitly changes the absolute value.
/// If subsequent increments result in a negative sum again, that will not lead
/// to an abort.
///
/// If a non-negative value is set, and subsequent increments added to that
/// value would be negative at the end of the transaction, it will revert.  A set of
/// invocations that conclude their transactions with the same absolute value,
/// and whose summed increments keep the value non-negative, may be included in
/// the same block.
///
pub fn non_negative_int_set(key: &StorageKey, set_value: i128) {
    let add_value = 0;

    nnint::set_add(key, Some(set_value), add_value);
}

/// Set the value exclusively.
///
/// This behaves like `non_negative_int_set`, with the additional constraint
/// that, if a concurrent exclusive-set operation is performed by another
/// contract invocation, only one such invocation can succeed without
/// reverting.
///
pub fn non_negative_int_set_exclusive(key: &StorageKey, set_value: i128) {
    let add_value = 0;

    nnint::set_add_exclusive(key, Some(set_value), add_value);
}

/// Increment the value.
///
/// Aborts if the resulting value overflows.
///
/// If a "set" operation has been used previously in this transaction and the
/// resulting value after incrementing by `amount` is negative, this aborts.
///
/// Otherwise, if the resulting value (the value at the end of the last
/// transaction plus `amount`) is negative, this function will not immediately
/// abort.  But note the transaction constraints at `non_negative_int_set`.
///
pub fn non_negative_int_inc(key: &StorageKey, amount: i128) {
    // Combine the existing "add" value with this additional increment
    let (s, a) = nnint::get_values(key);
    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    // Update the increment, keeping the "set" value (if any) as-is.
    nnint::set_add(key, s, add_value);
}

/// Increment the value.
///
/// Aborts if the resulting value overflows, or if it would be negative.
///
pub fn non_negative_int_strict_inc(key: &StorageKey, amount: i128) {
    let (s, a) = nnint::try_get_values_force(key).unwrap_or((0, 0));

    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    let total = s.checked_add(add_value).unwrap_or_else(|| nnint_overflow());

    if total < 0 {
        nnint_underflow();
    }

    nnint::set_add(key, Some(s), add_value);
}

/// Reset the value to the state it had at the start of the transaction, if
/// any.
///
/// If the address does not contain a non-negative int, this does not fail and
/// otherwise has the same effect as `non_negative_int_inc(key, 0)`.
///
pub fn non_negative_int_reset(key: &StorageKey) {
    nnint::set_add(key, None, 0);
}
