use super::keypath::DefaultFromKeypath;
use super::keypath::{KeyCache, Keypath};
use crate::bytestring::*;
use alloc::boxed::Box;
use alloc::vec;
use core::fmt::Debug;

#[derive(Debug)]
pub struct Bytestring {
    key: KeyCache,
}

impl Bytestring {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),
        }
    }

    pub fn len(&self) -> usize {
        bytestring_get_len(&self.key).unwrap_or(0) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set(&mut self, buf: &[u8]) {
        bytestring_set(&self.key, buf).unwrap();
    }

    /*
    pub fn set_exclusive(&mut self, buf: &[u8]) {
        bytestring_set_exclusive(&self.key, buf).unwrap()
    }
    */

    pub fn get_sized<const N: usize>(&self) -> Option<[u8; N]> {
        bytestring_get_sized(&self.key).unwrap()
    }

    #[cfg(feature = "alloc")]
    pub fn get(&self) -> Box<[u8]> {
        bytestring_get(&self.key).unwrap_or_else(|err| {
            if err.is_storage_key_not_exist() {
                vec![].into_boxed_slice()
            } else {
                panic!();
            }
        })
    }
}

impl From<Keypath> for Bytestring {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
        }
    }
}

impl DefaultFromKeypath for Bytestring {
    fn default_from_keypath(keypath: &Keypath) -> Self {
        Bytestring::new(keypath)
    }
}

impl_read_xdr_path_via_from! { Bytestring }
impl_write_xdr_empty! { Bytestring }
