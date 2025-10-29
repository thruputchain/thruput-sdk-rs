//! System functions: non-negative integers
//!
//! A non-negative integer has a "set" value determined at the start of the transaction and an
//! "add" value that is initially zero.  Both values may be modified during the transaction.  The
//! "current" value is the sum of "set" and "add".

use crate::sysresult::{Result, nnint_overflow};
use crate::types::StorageKey;
use crate::internal::nnint::*;

/// Get the "current" ("set" + "add") value.
pub fn non_negative_int_get(key: &StorageKey) -> Result<i128> {
    let (s, a) = get_values(key, true)?;

    // using `force_load_set = true` above ensures the "set" value is available
    let s = s.unwrap();

    let cur = s.checked_add(a).ok_or_else(nnint_overflow)?;

    Ok(cur)
}

pub fn non_negative_int_try_get(key: &StorageKey) -> Result<Option<i128>> {
    non_negative_int_get(key).map(Some).or_else(|e|
        if e.is_storage_key_not_exist() {
            Ok(None)
        } else {
            Err(e)
        })
}

/// Modify the "set" value and reset "add" to zero.
pub fn non_negative_int_set(
    key: &StorageKey,
    set_value: i128,
) -> Result<()> {
    let add_value = 0;

    set_add(key, Some(set_value), add_value)?;

    Ok(())
}

/// Increment the "add" value and preserve the "set" value.  (If this location was not previously
/// set in this transaction, the "set" value will remain what it was at the start of the
/// transaction.)
pub fn non_negative_int_inc(
    key: &StorageKey,
    amount: i128,
) -> Result<()> {
    let (s, a) = get_values(key, false)?;

    let add_value = a.checked_add(amount).unwrap();

    set_add(key, s, add_value)?;

    Ok(())
}

/// Revert the "set" value to its state at the start of the transaction, and clear the "add" value.
pub fn non_negative_int_revert(
    key: &StorageKey,
) -> Result<()> {
    set_add(key, None, 0)?;

    Ok(())
}
