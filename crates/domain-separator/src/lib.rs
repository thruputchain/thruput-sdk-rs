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
//! - `sha2::pubkey_account_address` hashes in-process. Host tooling.
//! - `syscall::pubkey_account_address` hashes through the SHA256 syscall.
//!   Contracts. Add `syscall-mock` to run it natively under `native-exec`.
//!
//! Both are named in plain code spans, not intra-doc links: each is gated on its
//! feature, and this crate is documented under whatever feature set the caller
//! picked, where a link to a cfg'd-out item is a hard error.
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
#[cfg(all(feature = "sha2", target_arch = "riscv32"))]
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
pub use separator::{DomainSeparator, PUBKEY_ACCOUNT, SIGNED_OP, TRANSACTION};
// Hidden macro surface: `domain_separator!` expands at its call site, so the
// helpers it names have to be reachable from crate root.
#[doc(hidden)]
pub use separator::{DomainSeparatorRepr, prefix_bytes, prefix_size};

mod preimage;

#[cfg(feature = "alloc")]
mod transaction;
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub use transaction::transaction_signature_preimage;

mod operation;
pub use operation::{
    SIGNED_OP_FIXED_PREFIX_LEN, SIGNED_OP_HEADER_MAX_LEN, SignedOp,
};

mod pubkey;
#[cfg(feature = "sha2")]
pub use pubkey::sha2;
#[cfg(feature = "syscall")]
pub use pubkey::syscall;
