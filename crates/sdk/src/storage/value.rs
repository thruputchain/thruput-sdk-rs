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
    key: LeafKey,
    _marker: PhantomData<fn() -> T>,
}

impl<T> super::node::StorageHandle for Value<T> {
    fn from_keypath(keypath: Keypath) -> Self {
        Self {
            key: LeafKey::from(keypath),
            _marker: PhantomData,
        }
    }
}

// Convenience: keeps `keypath.into()` working. The `#[storage]` derive
// bounds fields on `StorageHandle`, not this, so the field diagnostic is
// unaffected.
impl<T> From<Keypath> for Value<T> {
    fn from(keypath: Keypath) -> Self {
        <Self as super::node::StorageHandle>::from_keypath(keypath)
    }
}

impl<T> super::sealed::Sealed for Value<T> {}
impl<T> super::node::StorageType for Value<T> {}

impl<T> Value<T> {
    /// Return this value's storage key.
    pub fn key(&self) -> &crate::types::StorageKey {
        self.key.get()
    }
}

impl<T> Value<T>
where
    T: for<'a> xdrs::ReadXDR<'a>,
{
    /// Deserialize and return the stored value.
    /// Aborts if the key does not exist.
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    pub fn get(&self) -> T {
        use crate::re_throw::OrReThrow;
        self.try_get().or_re_throw()
    }

    /// Deserialize and return the stored value, reporting failure as `Err`
    /// instead of aborting (the `try_` law; see the [`exn`](mod@crate::exn)
    /// module docs).
    ///
    /// The `Err` is exactly the exception [`get`](Self::get) would have
    /// thrown: a missing key is a `LibraryExn::Syscall` for which
    /// [`is_storage_missing`](crate::exn::LibraryExn::is_storage_missing)
    /// is true — use
    /// [`none_if_missing`](crate::exn::NoneIfMissing::none_if_missing) to
    /// treat just that case as an ordinary `None`. A value that fails to
    /// decode as `T` (or exceeds the no-alloc read buffer — either way, the
    /// slot was written by something other than `Value<T>::set`) is
    /// `LibraryExn::StorageSerialization`; a value of another storage type
    /// or any other syscall failure is a `LibraryExn::Syscall`.
    pub fn try_get(&self) -> Result<T, crate::exn::LibraryExn> {
        use crate::exn::{LibraryExn, syscall_exn};
        use crate::sys::SyscallNo;
        use crate::sys_result::bytestring::bytestring_read_into;

        // Read into the reused in-place scratch and decode an owned value out
        // of it before the borrow ends — no buffer of our own.
        crate::call::with_scratch(|buf| {
            match bytestring_read_into(self.key.get(), buf) {
                Ok(Some(())) => {}
                // Stored value exceeds the buffer: only possible if the slot
                // was written by something other than Value<T>::set.
                Ok(None) => return Err(LibraryExn::StorageSerialization),
                Err(e) => {
                    return Err(syscall_exn(SyscallNo::kBytestringGet, e));
                }
            }
            xdrs::cxs::decode(buf.as_slice())
                .map_err(|_| LibraryExn::StorageSerialization)
        })
    }
}

impl<T> Value<T>
where
    T: xdrs::WriteXDR,
{
    /// Serialize and store the value immediately.
    pub fn set(&mut self, value: &T) {
        use crate::re_throw::OrReThrow;
        self.try_set(value).or_re_throw()
    }

    /// Like [`set`](Self::set), but report failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs). A value that fails to serialize is
    /// `LibraryExn::StorageSerialization`; a syscall failure is
    /// `LibraryExn::Syscall`. On `Err` the stored value is unmodified.
    pub fn try_set(
        &mut self,
        value: &T,
    ) -> Result<(), crate::exn::LibraryExn> {
        self.try_set_impl(value, self.key.is_exclusive())
    }

    /// Serialize and store the value with an exclusive lock.
    pub fn set_exclusive(&mut self, value: &T) {
        use crate::re_throw::OrReThrow;
        self.try_set_exclusive(value).or_re_throw()
    }

    /// Like [`set_exclusive`](Self::set_exclusive), but report failure as
    /// `Err` instead of aborting (the `try_` law; see the
    /// [`exn`](mod@crate::exn) module docs). The error contract is
    /// [`try_set`](Self::try_set)'s. On `Err` the stored value is
    /// unmodified and no lock is taken.
    pub fn try_set_exclusive(
        &mut self,
        value: &T,
    ) -> Result<(), crate::exn::LibraryExn> {
        self.try_set_impl(value, true)
    }

    /// Shared serialize-and-set; `exclusive` selects the lock flavor.
    fn try_set_impl(
        &mut self,
        value: &T,
        exclusive: bool,
    ) -> Result<(), crate::exn::LibraryExn> {
        use crate::exn::{LibraryExn, syscall_exn};
        use crate::sys::SyscallNo;
        use crate::sys_result::bytestring::*;

        crate::call::with_scratch(|buf| {
            crate::internal::encode_scratch(buf, value)
                .map_err(|_| LibraryExn::StorageSerialization)?;
            let result = if exclusive {
                bytestring_set_exclusive(&self.key, buf.as_slice())
            } else {
                bytestring_set(&self.key, buf.as_slice())
            };
            result.map_err(|e| syscall_exn(SyscallNo::kBytestringSet, e))
        })
    }
}

impl<T> Value<T>
where
    T: xdrs::WriteXDR + for<'a> xdrs::ReadXDR<'a>,
{
    /// Load the value and return a mutable reference to it.
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    pub fn load_mut(&mut self) -> ValueMut<'_, T> {
        let value = self.get();
        ValueMut {
            value: self,
            data: value,
            dirty: false,
            _live: crate::storage::live_guards::LiveGuard::new(),
        }
    }
}

impl<T> super::deletable::Deletable for Value<T> {
    fn delete_internal(&mut self, flags: StorageDeltaFlags) {
        let keypath_flags = self.key.flags();
        let effective = StorageDeltaFlags(flags.0 | keypath_flags.0);
        crate::sys_exn::delete::delete_with_flags(&self.key, effective);
    }
}

/// Mutable view into a `Value<T>`. Flushes `T` back to storage on drop if
/// it was mutated (accessed through `DerefMut`) since the last
/// [`flush`](Self::flush); a guard that was only read writes nothing.
pub struct ValueMut<'a, T: __xdrs::WriteXDR> {
    value: &'a mut Value<T>,
    data: T,
    dirty: bool,
    /// Registers this guard as live for the terminal-return check. Holding
    /// the field is the registration; on guest builds it is a `Drop`-less
    /// ZST and compiles away. Any future deferred-write guard should carry
    /// one too.
    _live: crate::storage::live_guards::LiveGuard,
}

impl<T: __xdrs::WriteXDR> ValueMut<'_, T> {
    /// Write the value back to storage now instead of waiting for drop.
    ///
    /// A guard that has not been mutated (no `DerefMut` access) since the
    /// last flush is a no-op. After a flush the guard remains usable;
    /// mutating it again re-arms the drop-time writeback.
    pub fn flush(&mut self) {
        use crate::re_throw::OrReThrow;
        self.try_flush().or_re_throw()
    }

    /// Like [`flush`](Self::flush), but report failure as `Err` instead of
    /// aborting (the `try_` law; see the [`exn`](mod@crate::exn) module
    /// docs) — in particular, the only way to catch an error the drop-time
    /// writeback would abort with. The `Err` is exactly
    /// [`Value::try_set`]'s. On `Err` the guard still holds the value as
    /// dirty: drop re-attempts the writeback and aborts if it still fails,
    /// so a caught error cannot silently lose the write.
    pub fn try_flush(&mut self) -> Result<(), crate::exn::LibraryExn> {
        if !self.dirty {
            return Ok(());
        }
        self.value.try_set(&self.data)?;
        self.dirty = false;
        Ok(())
    }
}

impl<T: __xdrs::WriteXDR> Deref for ValueMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: __xdrs::WriteXDR> DerefMut for ValueMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.data
    }
}

impl<T: __xdrs::WriteXDR> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        if self.dirty {
            self.value.set(&self.data);
        }
        // `_live` drops here, decrementing the count.
    }
}
