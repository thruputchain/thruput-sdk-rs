#![cfg_attr(not(feature = "dev"), no_std)]

thruput_sdk::declare_mock_contract!();

mod implementation;
mod interface;

pub use implementation::ImportedErrorCalleeContract;
pub use interface::ImportedErrorCallee;
pub use interface::ImportedErrorCalleeClient;

// Re-exported so a caller can name the thrown type — and so match it —
// through this crate's surface, without depending on where it is defined.
pub use imported_error_type::ImportedError;
