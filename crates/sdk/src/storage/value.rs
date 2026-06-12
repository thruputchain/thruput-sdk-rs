// src/storage/value.rs
use super::keypath::*;
use crate::internal::encode_xdr;
use core::marker::PhantomData;
pub use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A typed storage slot for serialized values.
///
/// `Value<T>` is a handle to a storage location that holds a serialized `T`.
/// All operations are immediate syscalls — no buffering.
/// For in-place modification with auto-flush, use `load_mut()`.
///
/// # Size cap
///
/// A stored value's encoding is capped at the SDK's fixed encode-buffer size
/// (~1 KiB). `set` throws a typed exception if the value exceeds that limit.
/// `get` and `try_get` throw `StorageSerialization` if the slot holds more
/// than that — which can only happen if something other than `Value<T>::set`
/// wrote the slot.
#[derive(Debug)]
pub struct Value<T> {
    key: KeyCache,
    _marker: PhantomData<fn() -> T>,
}

impl<T> From<Keypath> for Value<T> {
    fn from(keypath: Keypath) -> Self {
        Self {
            key: KeyCache::from(keypath),
            _marker: PhantomData,
        }
    }
}

impl<T> super::sealed::Sealed for Value<T> {}
impl<T> super::node::StorageType for Value<T> {}

impl<T> Value<T>
where
    T: for<'a> xdrs::ReadXDR<'a>,
{
    /// Deserialize and return the stored value.
    /// Aborts if the key does not exist.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn get(&self) -> T {
        use crate::exn::{LibraryExn, SyscallExn, library_throw};
        use crate::sys::SyscallNo;
        use crate::sys_result::bytestring::bytestring_get_encode_buf;

        // Using match instead of a closure keeps call-tracking (track_caller)
        // intact through to library_throw.
        let buf = match bytestring_get_encode_buf(self.key.get()) {
            Ok(Some(v)) => v,
            // Stored value exceeds the encode buffer: can only happen if the slot
            // was written by something other than Value<T>::set (which encodes
            // through the same fixed buffer).
            Ok(None) => library_throw(LibraryExn::StorageSerialization),
            Err(e) => library_throw(LibraryExn::Syscall(
                SyscallExn::new(SyscallNo::kBytestringGet, e),
            )),
        };
        match xdrs::cxs::decode(buf.as_ref()) {
            Ok(v) => v,
            Err(_) => library_throw(LibraryExn::StorageSerialization),
        }
    }

    /// Deserialize and return the stored value, or None if the key doesn't exist.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn try_get(&self) -> Option<T> {
        use crate::exn::{LibraryExn, SyscallExn, library_throw};
        use crate::sys::SyscallNo;
        use crate::sys_result::bytestring::bytestring_get_encode_buf;

        let buf = match bytestring_get_encode_buf(self.key.get()) {
            Ok(Some(v)) => v,
            Ok(None) => library_throw(LibraryExn::StorageSerialization),
            Err(e) if e.is_storage_key_not_exist() => return None,
            Err(e) => library_throw(LibraryExn::Syscall(
                SyscallExn::new(SyscallNo::kBytestringGet, e),
            )),
        };
        match xdrs::cxs::decode(buf.as_ref()) {
            Ok(v) => Some(v),
            Err(_) => library_throw(LibraryExn::StorageSerialization),
        }
    }
}

impl<T> Value<T>
where
    T: xdrs::WriteXDR,
{
    /// Serialize and store the value immediately.
    pub fn set(&mut self, value: &T) {
        use crate::sys_exn::bytestring::*;

        let buf = encode_xdr(value)
            .unwrap_or_else(|e| super::error::throw_store_exn(e));
        if self.key.keypath().is_exclusive() {
            bytestring_set_exclusive(&self.key, buf.as_ref());
        } else {
            bytestring_set(&self.key, buf.as_ref());
        }
    }

    /// Serialize and store the value with an exclusive lock.
    pub fn set_exclusive(&mut self, value: &T) {
        use crate::sys_exn::bytestring::*;

        let buf = encode_xdr(value)
            .unwrap_or_else(|e| super::error::throw_store_exn(e));
        bytestring_set_exclusive(&self.key, buf.as_ref());
    }
}

impl<T> Value<T>
where
    T: xdrs::WriteXDR + for<'a> xdrs::ReadXDR<'a>,
{
    /// Load the value and return a mutable reference to it.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn load_mut(&mut self) -> ValueMut<'_, T> {
        let value = self.get();
        ValueMut {
            value: self,
            data: value,
        }
    }
}

impl<T> super::deletable::Deletable for Value<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = if self.key.keypath().is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        };
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(&self.key, effective);
    }
}

/// Mutable view into a `Value<T>`. Flushes T back to storage on drop.
pub struct ValueMut<'a, T: __xdrs::WriteXDR> {
    value: &'a mut Value<T>,
    data: T,
}

impl<T: __xdrs::WriteXDR> Deref for ValueMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: __xdrs::WriteXDR> DerefMut for ValueMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T: __xdrs::WriteXDR> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        self.value.set(&self.data);
    }
}
