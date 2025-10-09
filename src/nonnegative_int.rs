//! System functions: non-negative integers
//!
//! A non-negative integer has a "set" value determined
//! at the start of the transaction, and an "add" value
//! that may be modified during the transaction.  The
//! "current" value is the sum of these.

use crate::sys;
use crate::sysresult::SysResult as Result;
use crate::types::StorageKeyContractData;

/// Get the "set" value (if requested by `force_load_set`) and the "add" value.
pub fn non_negative_int_get_values(
    key: &StorageKeyContractData,
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

    debug_assert!(force_load_set && set_loaded);

    let set_result_opt = if set_loaded { Some(set_result) } else { None };

    Ok((set_result_opt, add_result))
}

/// Get the "current" ("set" + "add") value.
pub fn non_negative_int_get(key: &StorageKeyContractData) -> Result<i128> {
    let (s, a) = non_negative_int_get_values(key, true)?;

    Ok(s.unwrap() + a)
}

/// Add an amount, optionally asserting the "set" value.
pub fn non_negative_int_set_add(
    key: &StorageKeyContractData,
    set_value: Option<i128>,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(key, set_value.as_ref(), &add_value)
}

/// Add an amount, after querying the "set" value.
pub fn non_negative_int_add(
    key: &StorageKeyContractData,
    amount: i128,
) -> Result<()> {
    let (s, a) = non_negative_int_get_values(key, false)?;

    non_negative_int_set_add(key, s, a + amount)
}

/// Add an amount, to zero if previously unset.
pub fn non_negative_int_add_default_zero(
    key: &StorageKeyContractData,
    amount: i128,
) -> Result<()> {
    let (s, a) = non_negative_int_get_values_default_zero(key)?;

    non_negative_int_set_add(key, Some(s), a + amount)
}

/// Get the "current" ("set" + "add") value, or zero if previously unset.
pub fn non_negative_int_get_default_zero(
    key: &StorageKeyContractData,
) -> Result<i128> {
    let (s, a) = non_negative_int_get_values_default_zero(key)?;

    Ok(s + a)
}

/// Get the "set" and "add" values, or zero if previously unset.
pub fn non_negative_int_get_values_default_zero(
    key: &StorageKeyContractData,
) -> Result<(i128, i128)> {
    match non_negative_int_get_values(key, true) {
        Ok((s, a)) => Ok((s.unwrap(), a)),
        Err(syserr) => {
            if syserr.is_storage_key_not_exist() {
                Ok((0, 0))
            } else {
                Err(syserr)
            }
        }
    }
}
