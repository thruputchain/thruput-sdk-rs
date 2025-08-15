//! System functions: non-negative integers

use crate::sysresult::SysResult as Result;
use crate::sys;
use crate::types::StorageKeyContractData;

pub fn non_negative_int_get_values(key: &StorageKeyContractData, force_load_set: bool) -> Result<(Option<i128>, i128)> {
    let mut set_result: i128 = 0;
    let mut add_result: i128 = 0;
    let mut set_loaded: bool = false;
    sys::raw_NonnegativeIntGet(key, force_load_set, &mut set_result, &mut set_loaded, &mut add_result)?;
    Ok((if set_loaded { Some(set_result) } else { None }, add_result))
}

pub fn non_negative_int_get(key: &StorageKeyContractData) -> Result<i128> {
    let (s, a) = non_negative_int_get_values(key, true)?;
    Ok(s.unwrap() + a)
}

// pub use non_negative_int_set_add as NonnegativeIntSetAdd;
pub fn non_negative_int_set_add(
    key: &StorageKeyContractData,
    set_value: Option<i128>,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(key, set_value.as_ref(), &add_value)
}

pub fn non_negative_int_add(key: &StorageKeyContractData, amount: i128) -> Result<()> {
    let (s, a) = non_negative_int_get_values(key, false)?;
    non_negative_int_set_add(key, s, a + amount)
}

/// Add an amount, to zero if key previously unset
pub fn non_negative_int_add_default_zero(
    key: &StorageKeyContractData,
    amount: i128,
) -> Result<()> {
    let (s, a) = non_negative_int_get_values_default_zero(key)?;
    non_negative_int_set_add(key, Some(s), a + amount)
}

/// Get the current value, or zero if key previously unset
pub fn non_negative_int_get_default_zero(key: &StorageKeyContractData) -> Result<i128> {
    let (s, a) = non_negative_int_get_values_default_zero(key)?;
    Ok(s + a)
}

/// Get the values, or zero if key previously unset
pub fn non_negative_int_get_values_default_zero(
    key: &StorageKeyContractData,
) -> Result<(i128, i128)> {
    // get_values with true will guarantee loading the set value,
    // so the unwrap is infallible
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
