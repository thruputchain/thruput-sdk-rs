#![cfg_attr(not(feature = "dev"), no_std)]

mod implementation;
mod interface;

pub use implementation::NamedInterfaceCalleeContract;
pub use interface::ExplicitNamedInterfaceClient;
pub use interface::ExplicitNamedInterfaceClientError;
pub use interface::NamedInterfaceCallee;
