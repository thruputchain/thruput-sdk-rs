//! Wrappers for the raw nnint syscalls

use crate::sysresult::SysResult as Result;
use crate::*;

/// Update (don't increment) the "set" (if provided) and "add" values.  If `set_value` is `None`,
/// the "set" value will be restored to its state at the start of the transaction.
pub fn set_add(
    key: &StorageKey,
    set_value: Option<i128>,
    add_value: i128,
) -> Result<()> {
    sys::raw_NonnegativeIntSetAdd(key, set_value.as_ref(), &add_value)
}

// Get the "set" value (either the latest set by the transaction or, if requested by
// `force_load_set`, the value ) and the "add" value.  Fails if `force_load_set` is true and the
// location does not contain a non-negative integer. (TODO: check semantics)
fn _get_values(
    key: &StorageKey,
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

    debug_assert!(if force_load_set { set_loaded } else { true });

    let set_result_opt = if set_loaded { Some(set_result) } else { None };

    Ok((set_result_opt, add_result))
}

pub fn get_values(
    key: &StorageKey,
) -> Result<(Option<i128>, i128)> {
    _get_values(key, false)
}

pub fn get_values_force(
    key: &StorageKey,
) -> Result<(i128, i128)> {
    let (s_opt, a) = _get_values(key, true)?;

    let s = s_opt.unwrap();

    Ok((s, a))
}

pub fn try_get_values(
    key: &StorageKey,
) -> Result<Option<(i128, i128)>> {
    match get_values_force(key) {
        Ok(values) => Ok(Some(values)),
        Err(e) if e.is_storage_key_not_exist() => Ok(None),
        Err(e) => Err(e),
    }
}
