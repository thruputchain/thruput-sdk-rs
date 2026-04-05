#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::CounterClient;
pub use implementation::CounterClientError;
pub use implementation::CounterEntrypoints;

// Counter type is re-exported so tests can construct clients directly.
pub use implementation::Counter;
