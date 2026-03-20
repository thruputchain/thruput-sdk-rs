//! Non-negative integers


/// Interface for objects representing non-negative integers in the key-value
/// store
pub trait IsNNInt {
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
}

mod nnint_storage_key {
    use crate::IsNNInt;
    use crate::sys_exn::nnint::*;
    use crate::StorageKey;

    #[derive(Debug)]
    pub struct NNInt(StorageKey);

    impl NNInt {
        pub const fn new(k: StorageKey) -> Self {
            Self(k)
        }
    }

    impl AsRef<StorageKey> for NNInt {
        fn as_ref(&self) -> &StorageKey {
            &self.0
        }
    }

    impl IsNNInt for NNInt {
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
            non_negative_int_set_exclusive(
                self.as_ref(),
                set_value,
            );
        }

        fn inc(&mut self, increment: i128) {
            non_negative_int_inc(self.as_ref(), increment);
        }

        fn strict_inc(&mut self, increment: i128) {
            non_negative_int_strict_inc(self.as_ref(), increment);
        }
    }
}
pub use nnint_storage_key::*;
