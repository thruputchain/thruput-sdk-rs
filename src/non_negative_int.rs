//! System functions: non-negative integers
//!
//! A non-negative integer has a value determined at the start of the transaction, and
//! can be re-set or incremented.

use crate::internal::nnint;
use crate::sysresult::{nnint_overflow, nnint_underflow};
use crate::types::StorageKey;

/// Get the "current" ("set" + "add") value.
pub fn non_negative_int_get(key: &StorageKey) -> i128 {
    let (s, a) = nnint::get_values_force(key);

    s.checked_add(a).unwrap_or_else(|| nnint_overflow())
}

/// Like `non_negative_int_get`, but returns `None` if the storage location
/// does not contain a non-negative-integer item.
pub fn non_negative_int_try_get(key: &StorageKey) -> Option<i128> {
    nnint::try_get_values_force(key)
        .map(|(s, a)| s.checked_add(a).unwrap_or_else(|| nnint_overflow()))
}

/// Set the value.
pub fn non_negative_int_set(key: &StorageKey, set_value: i128) {
    let add_value = 0;

    nnint::set_add(key, Some(set_value), add_value);
}

/// Set the value.
///
/// TODO: semantics
///
pub fn non_negative_int_set_exclusive(key: &StorageKey, set_value: i128) {
    let add_value = 0;

    nnint::set_add_exclusive(key, Some(set_value), add_value);
}

/// Increment the value.
///
/// If the result would be negative, this function may not fail, but the
/// current transaction will ultimately fail.
pub fn non_negative_int_inc(key: &StorageKey, amount: i128) {
    let (s, a) = nnint::get_values(key);

    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    nnint::set_add(key, s, add_value);
}

/// Increment the value.
///
/// Aborts if the result would be negative.
pub fn non_negative_int_strict_inc(key: &StorageKey, amount: i128) {
    let (s, a) = nnint::try_get_values_force(key).unwrap_or((0, 0));

    let add_value = a.checked_add(amount).unwrap_or_else(|| nnint_overflow());

    let total = s.checked_add(add_value).unwrap_or_else(|| nnint_overflow());

    if total < 0 {
        nnint_underflow();
    }

    nnint::set_add(key, Some(s), add_value);
}

/// Revert the value to its state at the start of the transaction.
pub fn non_negative_int_revert(key: &StorageKey) {
    nnint::set_add(key, None, 0);
}
