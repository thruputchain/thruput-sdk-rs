use super::error::{Error, Result};
use super::keypath::*;
use crate::bytestring::bytestring_get;
use crate::types::StorageKeyContractData;
use xdrs::ReadXDROwned;
use xdrs::serialize_cxs;

/// Must implement at least `load`
pub trait Load: Sized {
    fn load(key: &StorageKeyContractData) -> Result<Self>;

    fn try_load(key: &StorageKeyContractData) -> Result<Option<Self>> {
        match Self::load(key) {
            Ok(v) => Ok(Some(v)),
            Err(error) => {
                if error.is_storage_key_not_exist() {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
}

impl<T> Load for T
where
    T: ReadXDROwned
{
    fn load(key: &StorageKeyContractData) -> Result<Self> {
        let buf = bytestring_get(key)?;
        let val = serialize_cxs::decode(buf.as_ref()).map_err(|_|
            Error::Serialization)?;
        Ok(val)
    }
}

pub trait LoadKeypath: Sized {
    fn load_keypath(keypath: &Keypath) -> Result<Self>;

    fn try_load_keypath(keypath: &Keypath) -> Result<Option<Self>> {
        match Self::load_keypath(keypath) {
            Ok(v) => Ok(Some(v)),
            Err(error) => {
                if error.is_storage_key_not_exist() {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
}

impl<V> LoadKeypath for V
where
    V: Load,
{
    fn load_keypath(keypath: &Keypath) -> Result<Self> {
        let key = &keypath.generate_key();
        Self::load(key)
    }
}
