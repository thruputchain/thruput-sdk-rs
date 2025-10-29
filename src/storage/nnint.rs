use super::keypath::Keypath;
use crate::non_negative_int::*;
use crate::sysresult::Result;
use core::fmt::Debug;

/// Non-negative integer
///
/// Operations on this type are implemented by
/// [these functions](crate::non_negative_int).
///
#[derive(Debug)]
pub struct NNInt {
    keypath: Keypath,
}

impl From<Keypath> for NNInt {
    fn from(keypath: Keypath) -> Self {
        Self { keypath }
    }
}

impl_load_via_keypath! { NNInt }
impl_default_via_keypath! { NNInt }
impl_read_via_keypath! { NNInt }
impl_write_xdr_empty! { NNInt }

impl NNInt {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
        }
    }

    /// Get the current value.  Panics if location not yet set.
    pub fn get(&self) -> Result<i128> {
        non_negative_int_get(&self.keypath.generate_key())
    }

    /// Get the current value, or none if not yet set
    pub fn try_get(&self) -> Result<Option<i128>> {
        match self.get() {
            Ok(value) => Ok(Some(value)),
            Err(e) => {
                if e.is_storage_key_not_exist() {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Increment by an amount (without querying the current value).
    pub fn inc(&mut self, increment: i128) -> Result<()> {
        non_negative_int_inc(&self.keypath.generate_key(), increment)?;
        Ok(())
    }

    /// Increment_by an amount, failing in case of a negative result.
    pub fn checked_inc(&mut self, increment: i128) -> Result<()> {
        non_negative_int_checked_inc(&self.keypath.generate_key(), increment)?;
        Ok(())
    }
}
