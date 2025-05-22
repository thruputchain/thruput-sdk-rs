use super::error::Result;
use super::keypath::*;
use crate::bytestring::bytestring_get;
use crate::serialize::{self, DeserializeOwned};
use crate::types::StorageKeyContractData;

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
    T: DeserializeOwned,
{
    fn load(key: &StorageKeyContractData) -> Result<Self> {
        let buf = bytestring_get(key)?;
        let val = deserialize_owned(buf.as_ref())?;
        Ok(val)
    }
}

fn deserialize_owned<T>(buf: &[u8]) -> Result<T>
where
    T: DeserializeOwned,
{
    let val = serialize::from_bytes(buf)?;
    Ok(val)
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
