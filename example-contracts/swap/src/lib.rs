//! A port of UniswapV2 from Solidity to groundhog
//!
//! Refer to the UniswapV2 [whitepaper] and [v2-core] repository for details.
//!
//! [whitepaper](https://docs.uniswap.org/whitepaper.pdf)
//! [v2-core](https://github.com/Uniswap/v2-core)

#![allow(ambiguous_glob_reexports)]
// XXX: include trait name in generated internal method names to avoid collisions?
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

extern crate alloc;

pub(crate) mod interfaces;

// Contract interfaces
pub use interfaces::iswapv2_pair::*;

// An implementation of ISwapV2Pair;
mod swapv2_pair;

// Swap contract methods exposed with C ABI, for running as guest
pub use swapv2_pair::swap_exports::*;

// ERC20 methods exposed with C ABI, for running as guest
pub use swapv2_pair::erc20_exports::*;

#[cfg(feature = "dev")]
pub use swapv2_pair::SwapV2Pair;

// Re-export the ERC20 trait for `dyn ERC20` in tests
#[cfg(feature = "dev")]
pub use groundhog_erc20_if::ERC20;

mod bignum;
