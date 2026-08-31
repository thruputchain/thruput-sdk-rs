//! Internal utilities.
//!
//! Some are public because used in proc-macros.
//!
//! *These interfaces are not intended for use by SDK clients, and may change at any time.*

#[allow(unused)]
mod mu_buf;
mod serialize;
mod terminal_return;

#[allow(unused_imports)]
pub use mu_buf::*;
pub use serialize::*;
pub use terminal_return::*;

#[cfg(not(target_arch = "riscv32"))]
mod call_tracking_contexts;

#[cfg(not(target_arch = "riscv32"))]
pub use call_tracking_contexts::*;

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
pub const fn const_str_concat_bytes<
    const N: usize,
    const M: usize,
    const LEN: usize,
>(
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
pub const fn const_str_concat<
    const N: usize,
    const M: usize,
    const LEN: usize,
>(
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

/// Build the full version-mismatch diagnostic bytes: `prefix` + `'major', from 'full'` + `suffix`.
///
/// `OUT` must equal `P + 11 + M + F + S` (the middle segment is always 11 + major + full bytes).
/// For use from generated `#[contract]` code only.
#[doc(hidden)]
#[inline]
pub const fn const_iface_version_mismatch_diagnostic_bytes<
    const P: usize,
    const M: usize,
    const F: usize,
    const S: usize,
    const OUT: usize,
>(
    prefix: &str,
    trait_major: &str,
    trait_full: &str,
    suffix: &[u8; S],
) -> [u8; OUT] {
    assert!(
        prefix.len() == P && trait_major.len() == M && trait_full.len() == F
    );
    assert!(OUT == P + 11 + M + F + S);
    let pb = prefix.as_bytes();
    let ab = trait_major.as_bytes();
    let fb = trait_full.as_bytes();
    let mut out = [0u8; OUT];
    let mut i = 0usize;

    let mut j = 0usize;
    while j < P {
        out[i] = pb[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < M {
        out[i] = ab[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    let mid = b", from ";
    j = 0;
    while j < 7 {
        out[i] = mid[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < F {
        out[i] = fb[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    j = 0;
    while j < S {
        out[i] = suffix[j];
        i += 1;
        j += 1;
    }

    assert!(i == OUT);
    out
}

/// Build the full interface-name-mismatch diagnostic bytes:
/// `prefix` + `'impl' but the trait defines 'trait'` + `suffix`.
///
/// `OUT` must equal `P + 27 + I + T + S`.
/// For use from generated `#[contract]` code only.
#[doc(hidden)]
#[inline]
pub const fn const_iface_name_mismatch_diagnostic_bytes<
    const P: usize,
    const I: usize,
    const T: usize,
    const S: usize,
    const OUT: usize,
>(
    prefix: &str,
    impl_name: &str,
    trait_name: &str,
    suffix: &[u8; S],
) -> [u8; OUT] {
    assert!(
        prefix.len() == P && impl_name.len() == I && trait_name.len() == T
    );
    assert!(OUT == P + 27 + I + T + S);
    let pb = prefix.as_bytes();
    let ib = impl_name.as_bytes();
    let tb = trait_name.as_bytes();
    let mut out = [0u8; OUT];
    let mut i = 0usize;

    let mut j = 0usize;
    while j < P {
        out[i] = pb[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < I {
        out[i] = ib[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    let mid = b" but the trait defines ";
    j = 0;
    while j < 23 {
        out[i] = mid[j];
        i += 1;
        j += 1;
    }

    out[i] = b'\'';
    i += 1;
    j = 0;
    while j < T {
        out[i] = tb[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\'';
    i += 1;

    j = 0;
    while j < S {
        out[i] = suffix[j];
        i += 1;
        j += 1;
    }

    assert!(i == OUT);
    out
}

#[cfg(test)]
mod iface_version_mismatch_diagnostic_tests {
    use super::{
        const_iface_name_mismatch_diagnostic_bytes,
        const_iface_version_mismatch_diagnostic_bytes,
    };

    #[test]
    fn const_iface_version_mismatch_diagnostic_bytes_matches_expected_text() {
        const PREFIX: &str = "a: ";
        const MAJ: &str = "0";
        const FULL: &str = "0.0";
        const SUFFIX: &[u8; 2] = b" x";
        const OUT: [u8; 20] =
            const_iface_version_mismatch_diagnostic_bytes::<3, 1, 3, 2, 20>(
                PREFIX, MAJ, FULL, SUFFIX,
            );
        assert_eq!(
            core::str::from_utf8(&OUT).unwrap(),
            "a: '0', from '0.0' x"
        );
    }

    #[test]
    fn const_iface_name_mismatch_diagnostic_bytes_matches_expected_text() {
        const PREFIX: &str = "a: ";
        const IMPL: &str = "impl";
        const TRAIT: &str = "iface";
        const SUFFIX: &[u8; 2] = b" x";
        const OUT: [u8; 41] =
            const_iface_name_mismatch_diagnostic_bytes::<3, 4, 5, 2, 41>(
                PREFIX, IMPL, TRAIT, SUFFIX,
            );
        assert_eq!(
            core::str::from_utf8(&OUT).unwrap(),
            "a: 'impl' but the trait defines 'iface' x"
        );
    }
}
