/// Paths `#[derive(ContractTypeSpec)]` resolves through, mirroring what `sdk`
/// re-exports. Lets a crate below `sdk` derive its spec impls instead of
/// hand-writing them; see `groundhog_macros`'s `contract_type_spec(crate = ...)`
/// docs and `groundhog-u256`, which does the same.
#[doc(hidden)]
pub mod __sdk_paths {
    pub use groundhog_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}

/// Lowercase hex, the encoding every vector file uses for byte fields.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub mod domain_separator_vectors;
pub mod entrypoint_vectors;
pub mod signed_op_vectors;
pub mod xdr_vectors;
