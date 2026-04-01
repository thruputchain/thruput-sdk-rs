#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
mod interface;

pub use implementation::ImportedErrorCalleeContract;
pub use interface::ImportedErrorCallee;
pub use interface::ImportedErrorCalleeClient;
pub use interface::ImportedErrorCalleeClientError;
