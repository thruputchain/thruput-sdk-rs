//! Generate interface specs from a trait or impl that has
//! been annotated with the `contract` macro

mod config;
pub use config::Config;

mod contract;
pub use contract::ContractSrc;

pub mod entrypoint;
pub use entrypoint::*;

pub mod client;
