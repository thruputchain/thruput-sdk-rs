//! A port of UniswapV2 from Solidity to groundhog
//!
//! Refer to the UniswapV2 [whitepaper] and [v2-core] repository for details.
//!
//! [whitepaper](https://docs.uniswap.org/whitepaper.pdf)
//! [v2-core](https://github.com/Uniswap/v2-core)

// #![allow(ambiguous_glob_reexports)]
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

extern crate alloc;

// Interface
mod bignum;
pub mod events;
pub(crate) mod interfaces;

// Implementation
pub(crate) mod reserves;
mod swapv2_pair;

pub use interfaces::iswapv2_pair::*;

#[cfg(feature = "dev")]
pub use swapv2_pair::SwapV2Pair;

// Re-export the ERC20 trait for `dyn ERC20` in tests
#[cfg(feature = "dev")]
pub use groundhog_erc20_if::ERC20;
