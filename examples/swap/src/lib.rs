#![cfg_attr(not(feature = "mock"), no_std)]
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

// Swap contract methods exposed with C ABI, for running as guest
#[cfg(feature = "guest")]
pub use swapv2_pair::swap_exports::*;

// ERC20 methods exposed with C ABI, for running as guest
#[cfg(feature = "guest")]
pub use swapv2_pair::erc20_exports::*;

#[cfg(feature = "mock")]
pub use swapv2_pair::swap_test_setup;

#[cfg(feature = "mock")]
pub use swapv2_pair::erc20_test_setup;

mod bignum;
