#![no_std]
#![feature(strict_overflow_ops)]

extern crate alloc;

pub(crate) mod interfaces;

// Contract interfaces
pub use interfaces::iswapv2_pair::*;

// A client stub that provides the interface via cross-contract calls
#[cfg(feature = "client")]
pub use interfaces::iswapv2_pair::client::*;

// An implementation of the contract interface
#[cfg(feature = "guest")]
mod swapv2_pair;

// Contract methods exposed with C ABI, for running as guest
#[cfg(feature = "guest")]
pub use swapv2_pair::guest_exports::*;

// A helper for running tests
#[cfg(feature = "mock")]
pub use swapv2_pair::test_setup;

mod bignum;
