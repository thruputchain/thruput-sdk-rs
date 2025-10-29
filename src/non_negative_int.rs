//! System functions: non-negative integers
//!
//! A non-negative integer has a "set" value determined at the start of the transaction and an
//! "add" value that is initially zero.  Both values may be modified during the transaction.  The
//! "current" value is the sum of "set" and "add".

use crate::internal::nnint;
use crate::sysresult::{Result, nnint_overflow, nnint_underflow};
use crate::types::StorageKey;

/// Get the "current" ("set" + "add") value.
pub fn non_negative_int_get(key: &StorageKey) -> Result<i128> {
    let (s, a) = nnint::get_values_force(key)?;

    let cur = s.checked_add(a).ok_or_else(nnint_overflow)?;

    Ok(cur)
}

pub fn non_negative_int_try_get(key: &StorageKey) -> Result<Option<i128>> {
    non_negative_int_get(key).map(Some).or_else(|e| {
        if e.is_storage_key_not_exist() {
            Ok(None)
        } else {
            Err(e)
        }
    })
}

/// Set the value.
pub fn non_negative_int_set(key: &StorageKey, set_value: i128) -> Result<()> {
    let add_value = 0;

    nnint::set_add(key, Some(set_value), add_value)?;

    Ok(())
}

/// Increment the value.
pub fn non_negative_int_inc(key: &StorageKey, amount: i128) -> Result<()> {
    let (s, a) = nnint::get_values(key)?;

    let add_value = a.checked_add(amount).ok_or_else(nnint_overflow)?;

    nnint::set_add(key, s, add_value)?;

    Ok(())
}

/// Increment the value.  Fails if the result would be negative.
pub fn non_negative_int_checked_inc(key: &StorageKey, amount: i128) -> Result<()> {
    let (s, a) = nnint::try_get_values(key)?.unwrap_or((0, 0));

    let add_value = a.checked_add(amount).ok_or_else(nnint_overflow)?;

    let total = s.checked_add(add_value).ok_or_else(nnint_overflow)?;
    if total < 0 {
        nnint_underflow();
    }

    nnint::set_add(key, Some(s), add_value)?;

    Ok(())
}

/// Revert the value to its state at the start of the transaction.
pub fn non_negative_int_revert(key: &StorageKey) -> Result<()> {
    nnint::set_add(key, None, 0)?;

    Ok(())
}

// --------------- Deprecated exports ----------------------------------------

/// *Deprecated.*  Use `non_negative_int_try_get`.
pub fn non_negative_int_get_default_zero(key: &StorageKey) -> Result<i128> {
    Ok(non_negative_int_try_get(key)?.unwrap_or(0))
}

/// *Deprecated.*  Same as `non_negative_int_inc`.
pub fn non_negative_int_add_default_zero(
    key: &StorageKey,
    amount: i128,
) -> Result<()> {
    non_negative_int_inc(key, amount)
}
