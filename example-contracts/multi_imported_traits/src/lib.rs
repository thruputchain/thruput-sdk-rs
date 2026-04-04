#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

mod implementation;

pub use implementation::MultiImportedTraitsContract;
