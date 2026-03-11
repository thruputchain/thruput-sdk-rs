#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;

pub use implementation::CounterClient;
pub use implementation::exports::*;

#[cfg(feature = "dev")]
pub use implementation::test_setup;
