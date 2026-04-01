#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
mod interface;

pub use implementation::VersionedCalleeContract;
pub use interface::VersionedCallee;
pub use interface::VersionedCalleeClient;
pub use interface::VersionedCalleeClientError;
