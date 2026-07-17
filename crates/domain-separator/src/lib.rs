#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! Domain-separated hash derivation for Groundhog.
//!
//! Derives `sha256( cxs(DomainSeparator) || cxs(object) )`, matching the C++
//! validator. The domain-separator prefix is baked into rodata at compile time
//! (see [`PUBKEY_ACCOUNT`]).
//!
//! # Choosing a SHA-256 backend
//!
//! The `sha2` and `syscall` features add backends; they are additive and may
//! both be on, which is what lets one binary derive addresses host-side *and*
//! run a contract frame (`native-exec`) without the two configurations
//! fighting over feature unification. Because both can be live, nothing is
//! selected implicitly — callers name the backend in the path:
//!
//! - [`sha2::pubkey_account_address`] hashes in-process. Host tooling.
//! - [`syscall::pubkey_account_address`] hashes through the SHA256 syscall.
//!   Contracts. Add `syscall-mock` to run it natively under `native-exec`.
//!
//! There is deliberately no default and no fallback: a contract silently
//! linking `sha2` would be a correctness and code-size bug, so a guest build
//! that can reach the `sha2` backend fails to compile outright (below).

#[cfg(feature = "alloc")]
extern crate alloc;

// The `sha2` backend hashes in-process; on a guest that is both wrong (the
// chain's hash must come from the syscall) and expensive (a whole SHA-256
// implementation in a code-size-critical binary). Feature unification makes
// this reachable by accident -- one host-ish dependency anywhere in a guest's
// graph turning `sha2` on is enough -- so it is a hard error rather than a
// silent substitution. Guests want `syscall`.
#[cfg(all(
    feature = "sha2",
    any(target_family = "wasm", target_arch = "riscv32")
))]
compile_error!(
    "groundhog-domain-separator: the `sha2` backend must not be linked into a \
     guest contract; use the `syscall` backend"
);

mod backend;
#[cfg(feature = "sha2")]
pub use backend::Sha2;
pub use backend::Sha256Backend;
#[cfg(feature = "syscall")]
pub use backend::Syscall;

mod separator;
pub use separator::{DomainSeparator, PUBKEY_ACCOUNT};

mod preimage;

mod pubkey;
#[cfg(feature = "sha2")]
pub use pubkey::sha2;
#[cfg(feature = "syscall")]
pub use pubkey::syscall;
