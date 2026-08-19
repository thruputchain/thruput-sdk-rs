//! Fixture for the SDK's `unknown_method_exn` feature.
//!
//! The contract itself is deliberately trivial — one method. What it is for
//! is the method it *doesn't* have: calling any other entrypoint id must
//! abort with the typed `LibraryExn::UnknownMethod` rather than trapping the
//! frame, both under the rv32 dispatcher and under the native mock.

#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::Defined;
pub use implementation::DefinedClient;
