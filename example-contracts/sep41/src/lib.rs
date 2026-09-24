//! Reference SEP-41-style token, using Groundhog authorization and storage.
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();
pub use groundhog_sep41_if::*;
mod implementation;
pub use implementation::TokenContract;
