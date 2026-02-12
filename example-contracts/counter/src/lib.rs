#![cfg_attr(not(feature = "mock"), no_std)]

#[cfg(feature = "guest")]
mod implementation;

#[cfg(feature = "guest")]
pub use implementation::*;

#[cfg(feature = "mock")]
pub use implementation::test_setup;

#[cfg(feature = "client")]
pub use implementation::CounterClient;
