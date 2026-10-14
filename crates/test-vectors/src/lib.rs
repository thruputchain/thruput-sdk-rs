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
