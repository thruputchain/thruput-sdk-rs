//! # Cross-contract calls
//!
//! This module is mostly used by the `#[contract]` macro, which
//! wraps each method implementation:
//!
//! - Fetch encoded arguments from the VM and decode them
//! - Run the method body
//! - Encode the result and return it to the VM
//!
//! The contract macro also constructs "client" stub-objects which
//! can be used to call other contracts with the provided trait interface.
//!
//! Generated clients expose two call styles:
//! - `client.method(...)`: success-oriented path; typed exceptions are
//!   re-thrown and faults stay faults
//! - `client.try_method(...)`: inspection path;
//!   `Result<ReturnType, ClientError>`
//!
//! The procedure for a call is:
//!
//! - Encode arguments
//! - Call invoke syscall
//! - Decode result

mod call_result;
mod calls;
mod client;
mod contract_result;

pub use call_result::*;
pub use calls::*;
pub use client::*;
pub use contract_result::*;

#[doc(hidden)]
pub mod internal; // used by contract macro
