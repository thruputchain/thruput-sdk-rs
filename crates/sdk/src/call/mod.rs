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
//!   `Result<ReturnType, CallError>`, where a caught exception stays an
//!   untyped [`RawException`](crate::exn::RawException) to be matched with
//!   [`match_exn!`](crate::match_exn)
//!
//! The procedure for a call is:
//!
//! - Encode arguments
//! - Call invoke syscall
//! - Decode result

mod calls;
mod contract_client;
mod contract_result;
mod encode;
mod raw_call_error;

pub use calls::*;
pub use contract_client::*;
pub use contract_result::*;
pub use encode::*;
pub use raw_call_error::*;
