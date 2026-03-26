#![cfg_attr(not(feature = "dev"), no_std)]

mod interface;
pub use interface::*;

mod implementation;
pub use implementation::*;
