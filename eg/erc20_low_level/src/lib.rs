//! ERC20 low-level implementation

#![no_std]

pub mod interface;

// A client stub that provides the interface via cross-contract calls
#[cfg(feature = "client")]
pub use interface::client;

// An implementation of the contract interface
#[cfg(feature = "guest")]
mod implementation;

// Contract methods exposed with C ABI for running as guest
#[cfg(feature = "guest")]
pub use implementation::*;

#[cfg(test)]
mod tests;
