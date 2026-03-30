//! Internal utilities.
//!
//! Some are public because used in proc-macros.
//!
//! *These interfaces are not intended for use by SDK clients, and may change at any time.*

#[allow(unused)]
mod mu_buf;
mod serialize;
mod static_buf;
mod track;

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

/// Concatenate two UTF-8 byte arrays at compile time (no allocation).
///
/// `LEN` must equal `N + M`. A third parameter is required because `[u8; N + M]` needs
/// `generic_const_exprs` on stable Rust; callers pass `LEN` explicitly (e.g. `{ N + M }`).
///
/// For `&str` inputs with known lengths, use [`const_str_concat`].
#[inline]
pub const fn const_str_concat_bytes<const N: usize, const M: usize, const LEN: usize>(
    a: &[u8; N],
    b: &[u8; M],
) -> [u8; LEN] {
    assert!(N + M == LEN);
    let mut out = [0u8; LEN];
    let mut i = 0;
    while i < N {
        out[i] = a[i];
        i += 1;
    }
    while i < LEN {
        out[i] = b[i - N];
        i += 1;
    }
    out
}

/// Concatenate two strings at compile time (no allocation).
///
/// `N` and `M` must match `a.len()` and `b.len()`; `LEN` must equal `N + M`.
/// To obtain a `&str`, use `core::str::from_utf8(&bytes)` in a `const` context
/// (valid UTF-8 is preserved when both inputs are `&str`).
#[inline]
pub const fn const_str_concat<const N: usize, const M: usize, const LEN: usize>(
    a: &str,
    b: &str,
) -> [u8; LEN] {
    assert!(a.len() == N && b.len() == M);
    assert!(N + M == LEN);
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let mut out = [0u8; LEN];
    let mut i = 0;
    while i < N {
        out[i] = ab[i];
        i += 1;
    }
    while i < LEN {
        out[i] = bb[i - N];
        i += 1;
    }
    out
}

