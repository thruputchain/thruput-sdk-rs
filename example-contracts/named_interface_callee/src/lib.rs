#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;
mod interface;

pub use implementation::NamedInterfaceCalleeContract;
pub use interface::ExplicitNamedInterfaceClient;
pub use interface::NamedInterfaceCallee;
