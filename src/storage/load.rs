use super::error::{Result, StorageError, storage_syserr};
use super::keypath::*;
use super::read_xdr_path::*;
use crate::sys_result::bytestring::*;

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

pub fn load_keypath<T>(keypath: &Keypath) -> Result<T>
where
    T: LoadKeypath,
{
    LoadKeypath::load_keypath(keypath)
}

pub fn load_readable<T>(keypath: &Keypath) -> Result<T>
where
    T: ReadXDRPath,
{
    use xdrs::cxs::buf_reader::BufReader;

    let key = &keypath.generate_key();
    let buf = bytestring_get(key).map_err(storage_syserr)?;

    let xdr_reader = BufReader::new(buf.as_ref());
    let path = keypath.clone();
    let mut kpr = KeypathReader { xdr_reader, path };

    ReadXDRPath::read_xdr_path(&mut kpr)
        .map_err(|_| StorageError::Serialization)
}

impl_load_via_read! { bool, i16, u16, i32, u32, i64, u64, i128, u128, }

// LoadKeypath for commonly-used XDR types (Value<T> requires this)
const _: () = {
    use crate::storage::error::Result;
    use crate::storage::*;
    use crate::types::{
        ContractAddress, Ed25519PublicKey, Ed25519Signature, SHA256Hash,
    };

    impl LoadKeypath for ContractAddress {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            load_readable(keypath)
        }
    }

    impl LoadKeypath for Ed25519PublicKey {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            load_readable(keypath)
        }
    }

    impl LoadKeypath for Ed25519Signature {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            load_readable(keypath)
        }
    }

    impl LoadKeypath for SHA256Hash {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            load_readable(keypath)
        }
    }

    impl<T: for<'a> xdrs::ReadXDR<'a>> LoadKeypath for Option<T> {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            load_readable(keypath)
        }
    }
};
