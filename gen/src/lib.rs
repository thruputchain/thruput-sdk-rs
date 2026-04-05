//! Generate interfacing materials from a trait or impl that has
//! been annotated with the `contract` macro.  This crate produces:
//!
//! - Client-stub implementations for making contract calls
//! - Guest method stubs and external symbols for calling into the contract
//! - Test-registration code

pub mod util;

mod config;
pub use config::Config;

pub mod contract;
pub use contract::{ConfiguredContract, ContractSrc, Method};

pub mod entrypoint;
pub use entrypoint::*;

pub mod client;
pub mod exports;
pub mod spec;
pub mod spec_reg;
pub mod strip;
pub mod test_reg;
