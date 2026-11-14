//! A contract whose methods divide 64-bit and 128-bit integers the caller
//! chooses.
//!
//! RV32IM divides 32-bit values natively. Wider divisions call the routines
//! in `thruput-rv32-runtime`. The rv32 test checks their results against
//! host arithmetic, and bounds what a division costs.

#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;

pub use implementation::*;
