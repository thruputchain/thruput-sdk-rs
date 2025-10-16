//! ERC20 contract interface and implementation

#![cfg_attr(not(feature = "mock"), no_std)]

// An implementation of the contract interface
mod implementation;

// Contract methods exposed with C ABI for running as guest
#[cfg(feature = "guest")]
pub use implementation::guest_exports::*;

#[cfg(test)]
mod tests;

#[cfg(feature = "bottle-test")]
mod bottle_test;
#[cfg(feature = "bottle-test")]
pub use bottle_test::*;
