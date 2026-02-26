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
//! can be used to call other contracts with the provided trait interface.  The
//! procedure for calls is:
//!
//! - Encode arguments
//! - Call invoke syscall
//! - Decode result

mod call_result;
mod calls;
mod contract_result;
mod handle;

pub use call_result::*;
pub use calls::*;
pub use contract_result::*;
pub use handle::*;

#[doc(hidden)]
pub mod internal; // used by contract macro
