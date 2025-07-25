use super::error::Result as StorageResult;
use super::keypath::DefaultFromKeypath;
use super::keypath::Keypath;
use super::load::LoadKeypath;
use crate::nonnegative_int::*;
use crate::result::SysResult as Result;
use core::fmt::Debug;

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

    pub fn get_values(&self) -> Result<(i128, i128)> {
        let (s, a) = non_negative_int_get_values(&self.keypath.generate_key(), true)?;
        Ok((s.unwrap(), a))
    }

    pub fn get(&self) -> Result<i128> {
        let (set_value, add_value) = self.get_values()?;
        Ok(set_value + add_value)
    }

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

    pub fn try_get(&self) -> Result<Option<i128>> {
        match self.get() {
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

    pub fn add(&mut self, add_value: i128) -> Result<()> {
        non_negative_int_add(&self.keypath.generate_key(), add_value)?;
        Ok(())
    }

    pub fn add_default_zero(&mut self, add_value: i128) -> Result<()> {
        non_negative_int_add_default_zero(
            &self.keypath.generate_key(),
            add_value,
        )?;
        Ok(())
    }

    pub fn set_add(&mut self, set_value: i128, add_value: i128) -> Result<()> {
        non_negative_int_set_add(
            &self.keypath.generate_key(),
            Some(set_value),
            add_value,
        )?;
        Ok(())
    }
}

serialize_unit_impl!(NNInt);
deserialize_keypath_impl!(NNInt, "an NNInt");
