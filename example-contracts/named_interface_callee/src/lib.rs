#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
mod interface;

pub use implementation::NamedInterfaceCalleeContract;
pub use interface::NamedInterfaceCallee;
pub use interface::NamedInterfaceCalleeClient;
pub use interface::NamedInterfaceCalleeClientError;
