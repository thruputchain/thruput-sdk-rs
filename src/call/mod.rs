//! Wrapped contract calls
//!
//! - Encode args
//! - Call invoke syscall
//! - Decode result

mod contract_result;
mod call_result;
mod call;
mod handle;

pub use contract_result::*;
pub use call_result::*;
pub use call::*;
pub use handle::*;

#[doc(hidden)]
pub mod internal; // used by contract macro
