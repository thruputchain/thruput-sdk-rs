use super::keypath::{KeyCache, Keypath};
use super::{throw_load_exn, throw_store_exn};
use crate::StorageKey;
use crate::sys_exn::rcq::*;
use crate::xdr::*;
use core::fmt::Debug;
use core::marker::PhantomData;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A retro-causal queue.
///
/// Items of the contained type are encoded into raw queue entries before being
/// inserted, and decoded from raw queue entries after being retrieved.
///
/// Methods named `*_raw` and `*_sized` are provided to manipulate raw entry
/// contents, which may be arbitrary bytestrings.  However, `insert_raw` may
/// create entries that the typed methods cannot successfully decode.
///
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
    /// Retrieve the storage keypath of the RCQ item.
    pub fn keypath(&self) -> &Keypath {
        self.key.keypath()
    }

    /// Retrieve the storage key of the RCQ item.
    pub fn key(&self) -> &StorageKey {
        self.key.get()
    }
}

impl<T> RCQ<T> {
    /// Insert a queue entry containing raw bytes.
    pub fn insert_raw(&mut self, queue_entry: &[u8]) {
        retrocausal_queue_insert(self.key(), queue_entry);
    }

    /// Consume a fixed-size queue entry.
    pub fn consume_sized<const N: usize>(&mut self) -> [u8; N] {
        retrocausal_queue_consume_sized(self.key())
    }

    /// Try to consume a fixed-size queue entry.
    ///
    /// If an entry is present but does not have the expected size, an
    /// exception is thrown.
    ///
    pub fn try_consume_sized<const N: usize>(&mut self) -> Option<[u8; N]> {
        retrocausal_queue_try_consume_sized(self.key())
    }

    /// Consume a raw queue entry into a boxed slice.
    pub fn consume_raw(&mut self) -> Box<[u8]> {
        retrocausal_queue_get(self.key(), false)
    }

    /// Try to consume a raw queue entry into a boxed slice.
    pub fn try_consume_raw(&mut self) -> Option<Box<[u8]>> {
        retrocausal_queue_try_get(self.key(), false)
    }

    /// Retrieve a fixed-size queue entry without consuming it.
    pub fn peek_sized<const N: usize>(&self) -> [u8; N] {
        retrocausal_queue_peek_sized(self.key())
    }

    /// Try to retrieve a fixed-size queue entry without consuming it.
    ///
    /// If an entry is present but does not have the expected size, an
    /// exception is thrown.
    ///
    pub fn try_peek_sized<const N: usize>(&self) -> Option<[u8; N]> {
        retrocausal_queue_try_peek_sized(self.key())
    }

    /// Retrieve a raw queue entry into a boxed slice without consuming it.
    pub fn peek_raw(&self) -> Box<[u8]> {
        retrocausal_queue_get(self.key(), true)
    }

    /// Try to retrieve a raw queue entry into a boxed slice without consuming it.
    pub fn try_peek_raw(&self) -> Option<Box<[u8]>> {
        retrocausal_queue_try_get(self.key(), true)
    }
}

impl<T> RCQ<T>
where
    T: WriteXDR,
{
    /// Insert a queue entry containing the encoded item.
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
    /// Consume a queue entry and return the decoded item.
    pub fn consume(&mut self) -> T {
        let encoded = self.consume_raw();
        cxs::decode(encoded.as_ref()).unwrap_or_else(|e| throw_load_exn(e))
    }

    /// Try to consume a queue entry and return the decoded item.
    pub fn try_consume(&mut self) -> Option<T> {
        let encoded = self.try_consume_raw()?;
        let item = cxs::decode(encoded.as_ref())
            .unwrap_or_else(|e| throw_load_exn(e));
        Some(item)
    }

    /// Retrieve a queue entry and return the decoded item, without consuming the entry.
    pub fn peek(&self) -> T {
        let encoded = self.peek_raw();
        cxs::decode(encoded.as_ref()).unwrap_or_else(|e| throw_load_exn(e))
    }

    /// Try to retrieve a queue entry and return the decoded item, without consuming the entry.
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
