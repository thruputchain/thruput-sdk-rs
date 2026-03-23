#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
pub use implementation::exports::*;

pub use implementation::CalcClient;

#[cfg(feature = "dev")]
pub use implementation::test_setup;
