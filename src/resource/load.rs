use crate::bytestring::bytestring_get;
use crate::types::StorageKey;
use xdrs::ReadXDROwned;
use xdrs::serialize_cxs::decode as decode_xdr;
use super::error::*;
use super::keypath::*;

/// Must implement at least `load`
pub trait Load: Sized {
    fn load(key: &StorageKey) -> Result<Self>;

    fn try_load(key: &StorageKey) -> Result<Option<Self>> {
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
    T: ReadXDROwned,
{
    fn load(key: &StorageKey) -> Result<Self> {
        let buf = bytestring_get(key)?;
        let val = decode_xdr(buf.as_ref())?;
        Ok(val)
    }
}

pub trait LoadPath: Sized {
    fn load_path(keypath: &Keypath) -> Result<Self>;

    fn try_load_path(keypath: &Keypath) -> Result<Option<Self>>
    {
        match Self::load_path(keypath) {
            Ok(r) => Ok(Some(r)),
            Err(e) => if e.is_storage_key_not_exist() {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

impl<T: Load> LoadPath for T {
    fn load_path(keypath: &Keypath) -> Result<Self> {
        Load::load(&keypath.generate_key())
    }
}
