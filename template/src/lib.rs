#![no_std]

extern crate alloc;

// A trait describing the contract interface
pub mod interface;

// A client stub that provides the interface via cross-contract calls
#[cfg(feature = "client")]
pub use interface::client;

// An implementation of the contract interface
#[cfg(feature = "guest")]
pub mod implementation;

// Contract methods exposed with C ABI for running as guest
#[cfg(feature = "guest")]
pub use implementation::guest_exports::*;

#[cfg(feature = "mock")]
pub use implementation::test_setup;

#[cfg(test)]
mod tests;
