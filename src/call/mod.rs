//! Wrapped contract calls
//!
//! - Encode args
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
