use super::keypath::{Keypath, LeafKey};
use crate::StorageKey;
use crate::exn::LibraryExn;
use crate::re_throw::OrReThrow;
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
/// The `try_` forms report failure — including an empty queue, an `Err` for
/// which [`is_storage_missing`](LibraryExn::is_storage_missing) is true —
/// through their return value instead of aborting (the `try_` law; see the
/// [`exn`](mod@crate::exn) module docs). The plain forms abort with exactly
/// the exception their `try_` counterpart would have returned.
///
/// # Size cap
///
/// The host caps a queue entry at 256 bytes. That is well under the SDK's
/// encode buffer, so `insert` encodes first and the cap is enforced by the
/// insert syscall: an oversized entry aborts with the syscall's untyped
/// `LibraryExn::Syscall`, not a typed serialization exception. `consume` and
/// `peek` throw (and `try_consume` and `try_peek` return)
/// `StorageSerialization` if an entry fails to decode as `T`.
///
#[derive(Debug)]
pub struct RCQ<T> {
    key: LeafKey,
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

    /// Consume a fixed-size queue entry, reporting failure as `Err`.
    ///
    /// An entry of a different size is `LibraryExn::RetrocausalQueueEntryWrongSize`
    /// and is left in the queue.
    pub fn try_consume_sized<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], LibraryExn> {
        retrocausal_queue_try_consume_sized(self.key())
    }

    /// Consume a raw queue entry into a boxed slice.
    #[cfg(feature = "alloc")]
    pub fn consume_raw(&mut self) -> alloc::boxed::Box<[u8]> {
        self.try_consume_raw().or_re_throw()
    }

    /// Consume a raw queue entry into a boxed slice, reporting failure as
    /// `Err`.
    #[cfg(feature = "alloc")]
    pub fn try_consume_raw(
        &mut self,
    ) -> Result<alloc::boxed::Box<[u8]>, LibraryExn> {
        retrocausal_queue_try_consume(self.key())
    }

    /// Retrieve a fixed-size queue entry without consuming it.
    pub fn peek_sized<const N: usize>(&self) -> [u8; N] {
        retrocausal_queue_peek_sized(self.key())
    }

    /// Retrieve a fixed-size queue entry without consuming it, reporting
    /// failure as `Err`.
    ///
    /// An entry of a different size is `LibraryExn::RetrocausalQueueEntryWrongSize`.
    pub fn try_peek_sized<const N: usize>(
        &self,
    ) -> Result<[u8; N], LibraryExn> {
        retrocausal_queue_try_peek_sized(self.key())
    }

    /// Retrieve a raw queue entry into a boxed slice without consuming it.
    #[cfg(feature = "alloc")]
    pub fn peek_raw(&self) -> alloc::boxed::Box<[u8]> {
        self.try_peek_raw().or_re_throw()
    }

    /// Retrieve a raw queue entry into a boxed slice without consuming it,
    /// reporting failure as `Err`.
    #[cfg(feature = "alloc")]
    pub fn try_peek_raw(&self) -> Result<alloc::boxed::Box<[u8]>, LibraryExn> {
        retrocausal_queue_try_peek(self.key())
    }
}

impl<T> RCQ<T>
where
    T: WriteXDR,
{
    /// Insert a queue entry containing the encoded item.
    pub fn insert(&mut self, entry: &T) {
        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, entry)
                .unwrap_or_else(|e| super::error::throw_store_exn(e));
            self.insert_raw(buf.as_slice());
        });
    }
}

impl<T> RCQ<T>
where
    T: ReadXDROwned,
{
    /// Consume a queue entry and return the decoded item.
    pub fn consume(&mut self) -> T {
        // A single consuming read: a decode failure throws, and the trap
        // rolls the consume back, so the destructive read is unobservable.
        self.try_decode_entry(false).or_re_throw()
    }

    /// Consume a queue entry and return the decoded item, reporting failure
    /// as `Err`. An entry that fails to decode as `T` is
    /// `LibraryExn::StorageSerialization` and is left in the queue.
    pub fn try_consume(&mut self) -> Result<T, LibraryExn> {
        // Decode from a peek so a bad entry is reported without being
        // destroyed (matching try_consume_sized), then pop the validated
        // entry without copying it out again.
        let value = self.try_decode_entry(true)?;
        retrocausal_queue_try_discard(self.key())?;
        Ok(value)
    }

    /// Retrieve a queue entry and return the decoded item, without consuming the entry.
    pub fn peek(&self) -> T {
        self.try_peek().or_re_throw()
    }

    /// Retrieve a queue entry and return the decoded item without consuming
    /// the entry, reporting failure as `Err`. Same error contract as
    /// [`try_consume`](Self::try_consume).
    pub fn try_peek(&self) -> Result<T, LibraryExn> {
        self.try_decode_entry(true)
    }

    fn try_decode_entry(&self, peek: bool) -> Result<T, LibraryExn> {
        crate::call::with_scratch(|buf| {
            retrocausal_queue_try_read_into(self.key(), peek, buf)?;
            cxs::decode(buf.as_slice())
                .map_err(|_| LibraryExn::StorageSerialization)
        })
    }
}

impl<S> super::sealed::Sealed for RCQ<S> {}
impl<S> super::node::StorageType for RCQ<S> {}

impl<T> super::deletable::Deletable for RCQ<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(self.key(), effective);
    }
}
