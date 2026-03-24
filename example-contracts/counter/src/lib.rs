#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;

pub use implementation::CounterClient;
pub use implementation::exports::*;

// Counter type is re-exported so tests can use register_mock::<Counter, Counter>()
pub use implementation::Counter;
