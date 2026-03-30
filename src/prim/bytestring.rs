//! Bytestrings

/// Interface for objects representing bytestrings in the key-value store
pub trait IsBytestring {

    /// Returns the length of the bytestring.
    ///
    fn get_len(&self) -> u32;

    /// Sets the contents of the bytestring.
    ///
    /// If concurrent contract invocations set the location to the same bytestring,
    /// this will not cause any such invocations to abort.
    ///
    fn set(slice: &[u8]);

    fn set_exclusive(slice: &[u8]);

    /// Get the current value.  Aborts if location not yet set.
    fn get(&self) -> i128;

    /// Get the current value, or none if not yet set
    fn try_get(&self) -> Option<i128>;

    /// Set the value
    fn set(&mut self, set_value: i128);

    /// Set the value exclusively
    fn set_exclusive(&mut self, set_value: i128);

    /// Increment by an amount (without querying the current value).
    fn inc(&mut self, increment: i128);

    /// Increment_by an amount, aborting in case of a negative result.
    fn strict_inc(&mut self, increment: i128);

    /// Delete the non-negative-integer value
    fn delete(self);

    /// Delete the RCQ value exclusively
    fn delete_exclusive(self);
}

mod bytestring_storage_key {
    use crate::{StorageKey, IsBytestring};
    use crate::sys_exn::{ bytestring::*, delete::* };

    #[derive(Clone, Debug)]
    pub struct NNInt(StorageKey);

    impl Bytestring {
        pub const fn new(k: StorageKey) -> Self {
            Self(k)
        }
    }

    impl AsRef<StorageKey> for Bytestring {
        fn as_ref(&self) -> &StorageKey {
            &self.0
        }
    }

    impl IsBytestring for Bytestring {
        fn set_exclusive(&self, slice: &[u8]) {
            bytestring_set_exclusive(self.as_key(), slice);
        }

        fn get_len(&self) -> u32 {
            bytestring_get_len(self.as_key())
        }

        fn get(&self) -> i128 {
            non_negative_int_get(self.as_ref())
        }

        fn try_get(&self) -> Option<i128> {
            non_negative_int_try_get(self.as_ref())
        }

        fn set(&mut self, set_value: i128) {
            non_negative_int_set(self.as_ref(), set_value);
        }

        fn set_exclusive(&mut self, set_value: i128) {
            non_negative_int_set_exclusive(self.as_ref(), set_value);
        }

        fn inc(&mut self, increment: i128) {
            non_negative_int_inc(self.as_ref(), increment);
        }

        fn strict_inc(&mut self, increment: i128) {
            non_negative_int_strict_inc(self.as_ref(), increment);
        }

        fn delete(self) {
            delete_shared(self.as_ref());
        }

        fn delete_exclusive(self) {
            delete_exclusive(self.as_ref());
        }
    }
}
pub use nnint_storage_key::*;
