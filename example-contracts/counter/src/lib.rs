#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;

pub use implementation::CounterClient;
pub use implementation::exports::*;

// Counter type is re-exported so tests can use Counter::__register_mock
pub use implementation::Counter;
