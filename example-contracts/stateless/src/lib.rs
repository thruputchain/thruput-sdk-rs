#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;

pub use implementation::Stateless;
pub use implementation::StatelessClient;
pub use implementation::exports::*;

pub mod interface1;
pub mod interface2;

#[cfg(feature = "dev")]
pub use implementation::test_setup;
