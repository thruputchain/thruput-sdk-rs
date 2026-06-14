// src/storage/value.rs
use super::keypath::*;
use core::marker::PhantomData;
pub use core::ops::{Deref, DerefMut};
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// A typed storage slot for serialized values.
///
/// `Value<T>` is a handle to a storage location that holds a serialized `T`.
/// All operations are immediate syscalls — no buffering.
/// For in-place modification with auto-flush, use `load_mut()`.
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
        use crate::sys_result::bytestring::bytestring_read_into;

        // Read into the reused in-place scratch and decode an owned value out
        // of it before the borrow ends — no buffer of our own.
        crate::call::with_scratch(|buf| {
            match bytestring_read_into(self.key.get(), buf) {
                Ok(Some(())) => {}
                // Stored value exceeds the buffer: only possible if the slot
                // was written by something other than Value<T>::set.
                Ok(None) => library_throw(LibraryExn::StorageSerialization),
                Err(e) => library_throw(LibraryExn::Syscall(SyscallExn::new(
                    SyscallNo::kBytestringGet,
                    e,
                ))),
            }
            match xdrs::cxs::decode(buf.as_slice()) {
                Ok(v) => v,
                Err(_) => library_throw(LibraryExn::StorageSerialization),
            }
        })
    }

    /// Deserialize and return the stored value, or None if the key doesn't exist.
    #[cfg_attr(feature = "debugging", track_caller)]
    pub fn try_get(&self) -> Option<T> {
        use crate::exn::{LibraryExn, SyscallExn, library_throw};
        use crate::sys::SyscallNo;
        use crate::sys_result::bytestring::bytestring_read_into;

        crate::call::with_scratch(|buf| {
            match bytestring_read_into(self.key.get(), buf) {
                Ok(Some(())) => {}
                Ok(None) => library_throw(LibraryExn::StorageSerialization),
                Err(e) if e.is_storage_key_not_exist() => return None,
                Err(e) => library_throw(LibraryExn::Syscall(SyscallExn::new(
                    SyscallNo::kBytestringGet,
                    e,
                ))),
            }
            match xdrs::cxs::decode(buf.as_slice()) {
                Ok(v) => Some(v),
                Err(_) => library_throw(LibraryExn::StorageSerialization),
            }
        })
    }
}

impl<T> Value<T>
where
    T: xdrs::WriteXDR,
{
    /// Serialize and store the value immediately.
    pub fn set(&mut self, value: &T) {
        use crate::sys_exn::bytestring::*;

        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, value).unwrap();
            if self.key.keypath().is_exclusive() {
                bytestring_set_exclusive(&self.key, buf.as_slice());
            } else {
                bytestring_set(&self.key, buf.as_slice());
            }
        });
    }

    /// Serialize and store the value with an exclusive lock.
    pub fn set_exclusive(&mut self, value: &T) {
        use crate::sys_exn::bytestring::*;

        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, value).unwrap();
            bytestring_set_exclusive(&self.key, buf.as_slice());
        });
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
