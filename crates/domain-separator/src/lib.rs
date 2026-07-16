#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! Domain-separated hash derivation for Groundhog.
//!
//! Derives `sha256( cxs(DomainSeparator) || cxs(object) )`, matching the C++
//! validator. The domain-separator prefix is baked into rodata at compile time
//! (see [`domain_separator!`]); the SHA-256 backend is feature-selected
//! (`sha2` for host/native, `syscall` for guests) with no trait machinery.

#[cfg(feature = "alloc")]
extern crate alloc;

// Exactly one SHA-256 backend must be selected.
#[cfg(all(feature = "sha2", feature = "syscall"))]
compile_error!(
    "groundhog-domain-separator: enable exactly one SHA-256 backend \
     (`sha2` or `syscall`), not both"
);
#[cfg(not(any(feature = "sha2", feature = "syscall")))]
compile_error!(
    "groundhog-domain-separator: enable a SHA-256 backend \
     (`sha2` for host/native, `syscall` for guests)"
);

mod backend;
