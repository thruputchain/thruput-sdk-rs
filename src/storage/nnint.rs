use super::error::Result as StorageResult;
use super::keypath::DefaultFromKeypath;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use crate::nonnegative_int::*;
use crate::sysresult::SysResult as Result;
use core::fmt::Debug;

/// Non-negative integer
///
/// A non-negative integer has a "set" value determined
/// at the start of the transaction, and an "add" value
/// that may be modified during the transaction.  The
/// "current" value is the sum of these.
///
/// Operations on this type are implemented by
/// [these system functions](crate::nonnegative_int).
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

impl LoadKeypath for NNInt {
    fn load_keypath(keypath: &Keypath) -> StorageResult<Self> {
        // No I/O necessary
        Ok(NNInt::new(keypath))
    }
}

impl DefaultFromKeypath for NNInt {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        NNInt::new(keypath)
    }
}

impl NNInt {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
        }
    }

    /// Get the "set" and "add" values.
    pub fn get_values(&self) -> Result<(i128, i128)> {
        let (s, a) = non_negative_int_get_values(&self.keypath.generate_key(), true)?;
        Ok((s.unwrap(), a))
    }

    /// Get the "set" and "add" values, or `None` if previously unset.
    pub fn try_get_values(&self) -> Result<Option<(i128, i128)>> {
        match self.get_values() {
            Ok(values) => Ok(Some(values)),
            Err(e) => {
                if e.is_storage_key_not_exist() {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Get the "current" ("set" + "add") value.
    pub fn get(&self) -> Result<i128> {
        non_negative_int_get(&self.keypath.generate_key())
    }

    /// Get the "current" ("set" + "add") value, or `None` if previously unset.
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

    /// Add an amount (without querying the "set" value).
    pub fn add(&mut self, add_value: i128) -> Result<()> {
        non_negative_int_set_add(&self.keypath.generate_key(), None, add_value)?;
        Ok(())
    }

    /// Add an amount, asserting the "set" value.
    pub fn set_add(&mut self, set_value: i128, add_value: i128) -> Result<()> {
        non_negative_int_set_add(
            &self.keypath.generate_key(),
            Some(set_value),
            add_value,
        )?;
        Ok(())
    }

    /* Is there any point to this?
     *
    /// Add an amount after querying the "set" value, defaulting the latter to zero.
    pub fn add_default_zero(&mut self, add_value: i128) -> Result<()> {
        non_negative_int_add_default_zero(
            &self.keypath.generate_key(),
            add_value,
        )?;
        Ok(())
    }
    */
}

const _: () = {
    use xdrs::{WriteXDR, Writer, WriteResult};

    impl WriteXDR for NNInt {
        fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult {
            Ok(())
        }
    }
};

serialize_unit_impl!(NNInt);
deserialize_keypath_impl!(NNInt, "an NNInt");
