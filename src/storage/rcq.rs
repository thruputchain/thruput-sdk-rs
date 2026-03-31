#![allow(unused_imports)]

use super::keypath::{KeyCache, Keypath};
use super::nnint::NNInt;
use super::node::StorageType;
use super::slot::{SlotMut, SlotRef};
use super::{throw_load_exn, throw_store_exn};
use crate::StorageKey;
use crate::exn::*;
use crate::sys_exn::rcq::*;
use crate::throw;
use crate::xdr::cxs::{decode, encode};
use crate::xdr::*;
use core::fmt::Debug;
use core::marker::PhantomData;
use groundhog_xdr::storage_item::StorageDeltaFlags;

#[derive(Debug)]
pub struct RCQ<T> {
    key: KeyCache,
    phantom_entry: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for RCQ<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: keypath.into(),
            phantom_entry: PhantomData,
        }
    }
}

impl<T> RCQ<T> {
    pub fn keypath(&self) -> &Keypath {
        self.key.keypath()
    }

    pub fn key(&self) -> &StorageKey {
        self.key.get()
    }
}

impl<T> RCQ<T> {
    pub fn insert_raw(&mut self, queue_entry: &[u8]) {
        retrocausal_queue_insert(self.key(), queue_entry);
    }

    pub fn consume_sized<const N: usize>(&mut self) -> [u8; N] {
        retrocausal_queue_consume_sized(self.key())
    }

    pub fn try_consume_sized<const N: usize>(&mut self) -> Option<[u8; N]> {
        retrocausal_queue_try_consume_sized(self.key())
    }

    pub fn consume_raw(&mut self) -> Box<[u8]> {
        retrocausal_queue_get(self.key(), false)
    }

    pub fn try_consume_raw(&mut self) -> Option<Box<[u8]>> {
        retrocausal_queue_try_get(self.key(), false)
    }

    pub fn peek_sized<const N: usize>(&self) -> [u8; N] {
        retrocausal_queue_peek_sized(self.key())
    }

    pub fn try_peek_sized<const N: usize>(&self) -> Option<[u8; N]> {
        retrocausal_queue_try_peek_sized(self.key())
    }

    pub fn peek_raw(&self) -> Box<[u8]> {
        retrocausal_queue_get(self.key(), true)
    }

    pub fn try_peek_raw(&self) -> Option<Box<[u8]>> {
        retrocausal_queue_try_get(self.key(), true)
    }
}

impl<T> RCQ<T>
where
    T: WriteXDR,
{
    pub fn insert(&mut self, entry: &T) {
        let encoded =
            cxs::encode(entry).unwrap_or_else(|e| throw_store_exn(e));
        self.insert_raw(encoded.as_ref());
    }
}

impl<T> RCQ<T>
where
    T: ReadXDROwned,
{
    /// Retrieve the next available queue item, consuming the entry.
    pub fn consume(&mut self) -> T {
        let encoded = self.consume_raw();
        cxs::decode(encoded.as_ref()).unwrap_or_else(|e| throw_load_exn(e))
    }

    /// Try to retrieve the next available queue item, consuming the entry.
    pub fn try_consume(&mut self) -> Option<T> {
        let encoded = self.try_consume_raw()?;
        let item = cxs::decode(encoded.as_ref())
            .unwrap_or_else(|e| throw_load_exn(e));
        Some(item)
    }

    /// Retrieve the next available queue item without consuming the entry.
    pub fn peek(&self) -> T {
        let encoded = self.peek_raw();
        cxs::decode(encoded.as_ref()).unwrap_or_else(|e| throw_load_exn(e))
    }

    /// Try to retrieve the next available queue item without consuming the
    /// entry.
    pub fn try_peek(&self) -> Option<T> {
        let encoded = self.try_peek_raw()?;
        let item = cxs::decode(encoded.as_ref())
            .unwrap_or_else(|e| throw_load_exn(e));
        Some(item)
    }
}

impl<S> super::sealed::Sealed for RCQ<S> {}
impl<S> super::node::StorageType for RCQ<S> {}

impl<T> super::deletable::Deletable for RCQ<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.keypath().is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(self.key(), effective);
    }
}
