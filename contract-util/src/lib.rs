//! Miscellaneous utility functions for contract development

use groundhog_xdr::contract::ContractEntrypointID;
use sha2::Digest;
use syn::Signature;

/// Standard signature for a contract method.
///
/// We currently only depend on the method name, but this may change in future versions.
// TODO(geoff,dobog): in future versions
// we may consider adding the arguments and return type to the signature
// as a checksum, if there is a clear way to derive a standardized argument
// type string at proc-macro runtime.
pub struct NormalizedMethodSignature {
    method_name: String,
}

impl NormalizedMethodSignature {
    pub fn new(method: &Signature) -> Self {
        Self {
            method_name: method.ident.to_string(),
        }
    }

    pub fn get_signature_string(&self) -> String {
        self.method_name.clone()
    }
}

/// Extract the major version string from a semver version following Cargo's rules.
///
/// The major version is the leftmost non-zero component:
/// - `"1.2.3"` → `"1"` (major is non-zero)
/// - `"0.9.2"` → `"0.9"` (major is 0, minor is non-zero)
/// - `"0.0.2"` → `"0.0.2"` (0.0.z is always considered major)
pub fn cargo_major_version(version_str: &str) -> String {
    let parts: Vec<&str> = version_str.split('.').collect();

    match parts.as_slice() {
        [major, ..] if *major != "0" => major.to_string(),
        [major, minor, ..] if *minor != "0" => format!("{}.{}", major, minor),
        _ => version_str.to_string(),
    }
}

/// Derive a standard entrypoint ID for a contract method.
/// The interface name is a domain separator.
///
/// In the SDK macros, (i.e. `#[contract]`), when a method is derived from a trait,
/// the interface name is the trait name.  When a method is derived from a struct,
/// the interface name is the struct name.
///
/// For contracts not using the derive macros of the SDK, we would suggest using
/// some arbitrary domain separator here (such as the name of the contract).
pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> ContractEntrypointID {
    let major_version = cargo_major_version(version_str);
    let method_sig = method.get_signature_string();

    // Entrypoint ID = sha256(TRAIT_NAME || MAJOR_VERSION || METHOD_NAME || METHOD_SIGNATURE)[0..8]
    let mut hasher = sha2::Sha256::new();
    hasher.update(interface_name.unwrap_or("_unnamed_interface").as_bytes());
    hasher.update(major_version.as_bytes());
    hasher.update(method_sig.as_bytes());
    let result = hasher.finalize();

    let entrypoint_id: [u8; 8] = result[0..8].try_into().unwrap();
    ContractEntrypointID::from(entrypoint_id)
}

pub fn entrypoint_export_name(id: ContractEntrypointID) -> String {
    format!("{:016X}", u64::from_be_bytes(*id))
}

#[cfg(test)]
mod tests;
