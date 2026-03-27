//! Retro-causal queues

/// Interface for objects representing retro-causal-queues in the key-value
/// store
pub trait IsRCQ {
    fn insert<E>(&mut self, entry: &E)
    where
        E: AsRef<[u8]>;

    fn consume<const N: usize, T>(&mut self) -> T
    where
        T: From<[u8; N]>;

    fn consume_sized<const N: usize>(&mut self) -> [u8; N];

    fn peek_sized<const N: usize>(&self) -> [u8; N];

    #[cfg(feature = "alloc")]
    fn consume_max<const N: usize>(&mut self) -> crate::Vec<u8>;

    #[cfg(feature = "alloc")]
    fn peek_max<const N: usize>(&self) -> crate::Vec<u8>;
}

mod rcq_storage_key {
    use crate::IsRCQ;
    use crate::StorageKey;
    use crate::sys_exn::rcq::*;

    /// An implementation of `IsRCQ` that simply wraps a `StorageKey`.
    #[derive(Debug)]
    pub struct RCQ(StorageKey);

    impl AsRef<StorageKey> for RCQ {
        fn as_ref(&self) -> &StorageKey {
            &self.0
        }
    }

    impl RCQ {
        pub const fn new(key: StorageKey) -> Self {
            Self(key)
        }
    }

    impl IsRCQ for RCQ {
        fn insert<E>(&mut self, entry: &E)
        where
            E: AsRef<[u8]>,
        {
            retrocausal_queue_insert(self.as_ref(), entry.as_ref())
        }

        fn consume<const N: usize, T>(&mut self) -> T
        where
            T: From<[u8; N]>,
        {
            retrocausal_queue_consume_sized(self.as_ref()).into()
        }

        fn consume_sized<const N: usize>(&mut self) -> [u8; N] {
            retrocausal_queue_consume_sized(self.as_ref())
        }

        fn peek_sized<const N: usize>(&self) -> [u8; N] {
            retrocausal_queue_peek_sized(self.as_ref())
        }

        #[cfg(feature = "alloc")]
        fn consume_max<const N: usize>(&mut self) -> crate::Vec<u8> {
            let peek = false;
            retrocausal_queue_get_max::<N>(self.as_ref(), peek)
        }

        #[cfg(feature = "alloc")]
        fn peek_max<const N: usize>(&self) -> crate::Vec<u8> {
            let peek = true;
            retrocausal_queue_get_max::<N>(self.as_ref(), peek)
        }
    }
}
pub use rcq_storage_key::*;
