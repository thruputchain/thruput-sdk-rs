//! Cross-language test vectors for Thruput SDK types.
//!
//! Each module generates one family of vectors: XDR encodings, entrypoint
//! ids, domain separators, signed operations and public-key authorization.
//! SDKs in other languages replay these vectors to check that they agree
//! with this one.

/// Paths `#[derive(ContractTypeSpec)]` resolves through, mirroring what
/// `thruput-sdk` re-exports, so vector payload types can derive their spec
/// impls instead of hand-writing them.
#[doc(hidden)]
pub mod __sdk_paths {
    pub use thruput_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}

/// Lowercase hex, the encoding every vector file uses for byte fields.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub mod domain_separator_vectors;
pub mod entrypoint_vectors;
pub mod pubkey_auth_vectors;
pub mod signed_op_vectors;
pub mod xdr_vectors;
