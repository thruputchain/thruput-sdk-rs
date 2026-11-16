//! A contract whose methods call `memcpy`, `memmove` and `memset` with
//! lengths and offsets chosen by the caller.
//!
//! On rv32 those are the overrides in `thruput-rv32-runtime`. The rv32 test
//! checks their results against a byte-at-a-time model, and bounds what each
//! costs per byte.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use implementation::*;
