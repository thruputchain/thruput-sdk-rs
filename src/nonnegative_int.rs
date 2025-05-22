#![allow(non_snake_case)]

use crate::result::SysResult as Result;
use crate::sys;
use crate::types::StorageKeyContractData;

pub fn non_negative_int_get_values(key: &StorageKeyContractData) -> Result<(i128, i128)> {
    let mut set_result: i128 = 0;
    let mut add_result: i128 = 0;
    sys::raw_NonnegativeIntGet(key, &mut set_result, &mut add_result)?;
    Ok((set_result, add_result))
}

pub fn non_negative_int_get(key: &StorageKeyContractData) -> Result<i128> {
    let (s, a) = non_negative_int_get_values(key)?;
    Ok(s + a)
}

// pub use non_negative_int_set_add as NonnegativeIntSetAdd;
pub fn non_negative_int_set_add(
    key: &StorageKeyContractData,
    set_value: i128,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(key, &set_value, &add_value)
}

pub fn non_negative_int_add(key: &StorageKeyContractData, amount: i128) -> Result<()> {
    let (s, a) = non_negative_int_get_values(key)?;
    non_negative_int_set_add(key, s, a + amount)
}

/// Add an amount, to zero if key previously unset
pub fn non_negative_int_add_default_zero(
    key: &StorageKeyContractData,
    amount: i128,
) -> Result<()> {
    let (s, a) = non_negative_int_get_values_default_zero(key)?;
    non_negative_int_set_add(key, s, a + amount)
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
    non_negative_int_get_values(key).or_else(|syserr| {
        if syserr.is_storage_key_not_exist() {
            Ok((0, 0))
        } else {
            Err(syserr)
        }
    })
}
