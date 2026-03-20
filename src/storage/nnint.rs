use super::keypath::Keypath;
use crate::non_negative_int::*;
use core::fmt::Debug;

/// Non-negative integer
///
/// Operations on this type are implemented by
/// [these functions](crate::non_negative_int).
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

impl_load_via_keypath! { NNInt }
impl_default_via_keypath! { NNInt }
impl_read_via_keypath! { NNInt }
impl_write_xdr_empty! { NNInt }

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

    /// Set the value
    pub fn set(&mut self, set_value: i128) {
        non_negative_int_set(&self.keypath.generate_key(), set_value);
    }

    /// Set the value exclusively
    pub fn set_exclusive(&mut self, set_value: i128) {
        non_negative_int_set_exclusive(
            &self.keypath.generate_key(),
            set_value,
        );
    }

    /// Increment by an amount (without querying the current value).
    pub fn inc(&mut self, increment: i128) {
        non_negative_int_inc(&self.keypath.generate_key(), increment);
    }

    /// Increment_by an amount, aborting in case of a negative result.
    pub fn strict_inc(&mut self, increment: i128) {
        non_negative_int_strict_inc(&self.keypath.generate_key(), increment);
    }
}
