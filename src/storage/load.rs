use super::error::Result;
use super::keypath::*;
use crate::bytestring::bytestring_get;

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
    T: LoadKeypath
{
    LoadKeypath::load_keypath(keypath)
}

const _: () = {
    use super::error::{Error, Result};
    use super::read_xdr_path::*;
    use xdrs::serialize_cxs::buf_reader::BufReader;

    impl<V> LoadKeypath for V
    where
        V: ReadXDRPath
    {
        fn load_keypath(keypath: &Keypath) -> Result<Self> {
            let key = &keypath.generate_key();
            let buf = bytestring_get(key)?;

            let xdr_reader = BufReader::new(buf.as_ref());
            let path = keypath.clone();
            let mut kpr = KeypathReader { xdr_reader, path };

            ReadXDRPath::read_xdr_path(&mut kpr).map_err(|_|
                Error::Serialization)
        }
    }
};
