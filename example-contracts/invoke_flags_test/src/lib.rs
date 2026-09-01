#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::KeyValueStore;
pub use implementation::KeyValueStoreClient;
pub use implementation::KeyValueStoreEntrypoints;
