//! System functions: non-negative integers
//!
//! At the start of a transaction, a storage location holds a non-negative-int
//! value if a previous transaction perfomed a "set" operation there, and the
//! most recent such operation placed a value of the non-negative-int storage
//! type. Otherwise, it does not contain a non-negative-int value.
//!
//! If the location does not contain a non-negative-int value, it may be set or
//! incremented, after which it has a value in the current transaction.  An
//! increment operation behaves as if there had been a previous value of zero.
//!
//! If the location _does_ have a non-negative-int value, it may be set,
//! incremented, or retrieved (by `non_negative_int_get`).
//!

use crate::exn::{nnint_overflow, nnint_underflow};
use crate::internal::nnint;
use crate::types::StorageKey;

/// Get the current ("set" + "add") value.
///
/// Aborts if the location does not contain a non-negative-int value.
///
pub fn non_negative_int_get(key: &StorageKey) -> i128 {
    let (s, a) = nnint::get_values_force(key);

    s.checked_add(a).unwrap_or_else(|| nnint_overflow())
}

/// Get the current ("set" + "add") value, or `None` if empty.
///
/// This behaves like `non_negative_int_get`, but returns `None` if the storage
/// location is empty or aborts of a value of the wrong type is there.
///
pub fn non_negative_int_try_get(key: &StorageKey) -> Option<i128> {
    nnint::try_get_values_force(key)
        .map(|(s, a)| s.checked_add(a).unwrap_or_else(|| nnint_overflow()))
}

/// Set the value.
///
/// A negative value is allowed, as this explicitly changes the absolute value.
///
/// This call always succeeds, but if the value plus any subsequent increments
/// in this contract invocation (plus increments in concurrent invocations with
/// the same explicitly-set value) would be negative at the end of the
/// transaction, it will revert.
///
pub fn non_negative_int_set(key: &StorageKey, set_value: i128) {
    let add_value = 0;

    nnint::set_add(key, Some(set_value), add_value);
}

/// Set the value.
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
/// XXX(NNInt): The above-noted behavior is the result of, e.g.,
/// `set_add(Some(3), -5)` causing an error.
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

/// Restore the value to the state it had at the start of the transaction.
///
/// If the address does not contain a non-negative int and the location has not
/// yet been loaded with `non_negative_int_get`, this does not fail and
/// otherwise has the same effect as `non_negative_int_inc(key, 0)`.
/// XXX(NNInt): Is this okay?
///
pub fn non_negative_int_restore(key: &StorageKey) {
    nnint::set_add(key, None, 0);
}
