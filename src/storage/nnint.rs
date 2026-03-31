use super::keypath::Keypath;
use crate::exn::{nnint_overflow, nnint_underflow};
use crate::sys_exn::nnint::*;
use crate::sys_exn::nnint_internal as nnint;
use core::fmt::Debug;
use groundhog_xdr::storage_item::StorageDeltaFlags;

/// Non-negative integer
///
#[derive(Debug)]
pub struct NNInt {
    keypath: Keypath,
}

impl From<Keypath> for NNInt {
    fn from(keypath: Keypath) -> Self {
        Self { keypath }
    }
}

impl NNInt {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
        }
    }

    /// Get the current value.  Aborts if location not yet set.
    pub fn get(&self) -> i128 {
        non_negative_int_get(&self.keypath.generate_key())
    }

    /// Get the current value, or none if not yet set
    pub fn try_get(&self) -> Option<i128> {
        non_negative_int_try_get(&self.keypath.generate_key())
    }

    fn flags(&self) -> StorageDeltaFlags {
        if self.keypath.is_exclusive() {
            StorageDeltaFlags::ExclusiveLock
        } else {
            StorageDeltaFlags::None
        }
    }

    /// Set the value
    pub fn set(&mut self, set_value: i128) {
        nnint::set_add(
            &self.keypath.generate_key(),
            Some(set_value),
            0,
            self.flags(),
        );
    }

    /// Increment by an amount (without querying the current value).
    pub fn inc(&mut self, increment: i128) {
        let key = &self.keypath.generate_key();
        let (s, a) = nnint::get_values(key);
        let add_value =
            a.checked_add(increment).unwrap_or_else(|| nnint_overflow());
        nnint::set_add(key, s, add_value, self.flags());
    }

    /// Increment by an amount, aborting in case of a negative result.
    pub fn strict_inc(&mut self, increment: i128) {
        let key = &self.keypath.generate_key();
        let (s, a) = nnint::try_get_values_force(key).unwrap_or((0, 0));
        let add_value =
            a.checked_add(increment).unwrap_or_else(|| nnint_overflow());
        let total =
            s.checked_add(add_value).unwrap_or_else(|| nnint_overflow());
        if total < 0 {
            nnint_underflow();
        }
        nnint::set_add(key, Some(s), add_value, self.flags());
    }

    /// Delete the value at this storage location.
    pub fn delete(&mut self) {
        let key = &self.keypath.generate_key();
        if self.keypath.is_exclusive() {
            crate::delete_exclusive(key);
        } else {
            crate::delete_shared(key);
        }
    }
}

impl super::sealed::Sealed for NNInt {}
impl super::node::StorageType for NNInt {}
