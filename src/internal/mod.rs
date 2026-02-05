//! Internal utilities used in proc-macros.
//!
//! *These interfaces are not intended for use by SDK clients, and may change at any time.*

#[allow(unused)]
mod mu_buf;
mod serialize;
mod static_buf;
mod track;

pub mod nnint;

#[allow(unused_imports)]
pub use mu_buf::*;
pub use serialize::*;
pub use track::*;

/// Constant-time string equality check for use in `const` assertions.
pub const fn const_str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
