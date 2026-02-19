//! Wrapped contract calls
//!
//! - Encode args
//! - Call invoke syscall
//! - Decode result

mod call;
mod call_result;
mod contract_result;
mod handle;

pub use call::*;
pub use call_result::*;
pub use contract_result::*;
pub use handle::*;

#[doc(hidden)]
pub mod internal; // used by contract macro
