//! Unstable development utilities that must be public
//! for use by proc-macros.
//!
//! *These interfaces may change at any time.*

mod handle_call;
mod panic;

pub use handle_call::handle_call;

///  Initialize internal development utilities.
pub fn init() {
    panic::init();
}
