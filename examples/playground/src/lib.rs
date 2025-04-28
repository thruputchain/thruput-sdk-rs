//! ERC20 contract interface and implementation

#![no_std]

// A trait describing the contract interface
pub mod interface;

// A client stub that provides the interface via cross-contract calls
#[cfg(feature = "client")]
pub use interface::client;

// An implementation of the contract interface
#[cfg(feature = "guest")]
mod implementation;

// Contract methods exposed with C ABI for running as guest
#[cfg(feature = "guest")]
pub use implementation::guest_exports::*;

#[cfg(test)]
mod tests;
