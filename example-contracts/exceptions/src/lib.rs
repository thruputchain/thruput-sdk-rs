#![cfg_attr(not(feature = "dev"), no_std)]

// The contract interface
mod interface;
pub use interface::*;

// An implementation of the contract interface
mod implementation;
pub use implementation::*;
