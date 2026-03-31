use super::keypath::{KeyCache, Keypath};
use crate::sys_exn::rcq::*;
use core::fmt::Debug;

#[derive(Debug)]
pub struct RCQ {
    key: KeyCache,
}

impl From<Keypath> for RCQ {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
        }
    }
}

impl_load_via_keypath! { RCQ }
impl_read_via_keypath! { RCQ }

impl super::sealed::Sealed for RCQ {}
impl super::node::StorageType for RCQ {}

impl RCQ {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            key: KeyCache::new(keypath),
        }
    }

    pub fn insert(&mut self, queue_entry: &[u8]) {
        retrocausal_queue_insert(&self.key, queue_entry);
    }

    pub fn delete(&mut self) {
        if self.key.keypath().is_exclusive() {
            crate::delete_exclusive(&self.key)
        } else {
            crate::delete_shared(&self.key)
        }
    }

    pub fn consume_sized<const N: usize>(&mut self) -> [u8; N] {
        retrocausal_queue_consume_sized(&self.key)
    }

    pub fn try_consume_sized<const N: usize>(&mut self) -> Option<[u8; N]> {
        retrocausal_queue_try_consume_sized(&self.key)
    }

    pub fn peek_sized<const N: usize>(&self) -> [u8; N] {
        retrocausal_queue_peek_sized(&self.key)
    }

    /// Get the next queue entry's data.
    ///
    /// A vector of maximum length `N` is returned.
    /// If `peek` is true, the entry will not be consumed.
    ///
    #[cfg(feature = "alloc")]
    pub fn get_max<const N: usize>(&mut self, peek: bool) -> crate::Vec<u8> {
        retrocausal_queue_get_max::<N>(&self.key, peek)
    }
}

