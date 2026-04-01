use super::keypath::{KeyCache, Keypath};
use crate::sys_exn::bytestring::*;
use alloc::boxed::Box;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

#[derive(Debug)]
pub struct Bytestring {
    key: KeyCache,
}

impl From<Keypath> for Bytestring {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
        }
    }
}

impl Bytestring {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),
        }
    }

    pub fn len(&self) -> usize {
        bytestring_get_len(&self.key) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set(&mut self, buf: &[u8]) {
        if self.key.keypath().is_exclusive() {
            bytestring_set_exclusive(&self.key, buf);
        } else {
            bytestring_set(&self.key, buf);
        }
    }

    pub fn get_sized<const N: usize>(&self) -> [u8; N] {
        bytestring_get_sized(&self.key)
    }

    pub fn get(&self) -> Box<[u8]> {
        bytestring_get(&self.key)
    }

    pub fn try_get(&self) -> Option<Box<[u8]>> {
        bytestring_try_get(&self.key)
    }
}

impl super::sealed::Sealed for Bytestring {}
impl super::node::StorageType for Bytestring {}

impl super::deletable::Deletable for Bytestring {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.keypath().is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(self.key.get(), effective);
    }
}
