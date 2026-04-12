//! Miscellaneous utility functions for contract development

use groundhog_xdr::contract::ContractEntrypointID;
use sha2::Digest;
use std::fmt::{self, Display};

pub mod semver;

/// Standard signature for a contract method.
///
/// We currently only depend on the method name, but this may change in future versions.
// TODO(geoff,dobog): in future versions
// we may consider adding the arguments and return type to the signature
// as a checksum, if there is a clear way to derive a standardized argument
// type string at proc-macro runtime.
//
#[derive(Debug)]
pub struct NormalizedMethodSignature {
    method_name: String,
}

impl NormalizedMethodSignature {
    #[cfg(feature = "syn")]
    pub fn new_from_signature(method: &syn::Signature) -> Self {
        Self {
            method_name: method.ident.to_string(),
        }
    }

    pub fn new(method_name: &str) -> Self {
        Self {
            method_name: method_name.to_string(),
        }
    }

    pub fn get_signature_string(&self) -> String {
        self.method_name.clone()
    }
}

impl Display for NormalizedMethodSignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.method_name.as_str())
    }
}

#[derive(Debug)]
pub struct EntrypointSignature {
    pub interface_name: Option<String>,
    pub version_str: String,
    pub method_sig: NormalizedMethodSignature,
}

impl EntrypointSignature {
    #[cfg(feature = "syn")]
    pub fn new(
        interface_name: Option<&str>,
        version_str: &str,
        method: &syn::Signature,
    ) -> Self {
        Self {
            interface_name: interface_name.map(|s| s.to_string()),
            version_str: version_str.to_string(),
            method_sig: NormalizedMethodSignature::new_from_signature(method),
        }
    }

    pub fn id(&self) -> ContractEntrypointID {
        contract_entrypoint_id(
            self.interface_name.as_deref(),
            self.version_str.as_str(),
            &self.method_sig,
        )
    }
}

impl Display for EntrypointSignature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}<{}>::{}",
            self.interface_name
                .as_deref()
                .unwrap_or("_unnamed_interface"),
            self.version_str,
            self.method_sig,
        )
    }
}

/// Derive a standard entrypoint ID for a contract method.
/// The interface name is a domain separator.
///
/// In the SDK macros (`#[contract]`):
/// - trait and trait-impl methods use the effective interface name
///   (`interface = "..."` override or the trait name)
/// - bare impl methods use the effective interface name
///   (`interface = "..."` override or the self type name)
/// - free functions default to `_unnamed_interface` unless an explicit
///   `interface = "..."` override is present
pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> ContractEntrypointID {
    let major_version = semver::major_version_from_string(version_str);
    let method_sig = method.get_signature_string();

    // Entrypoint ID = sha256(INTERFACE_NAME || MAJOR_VERSION || METHOD_SIGNATURE)[0..8]
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

/// Derive a standard exception selector for a contract exception type.
///
/// Exception selector = sha256(MAJOR_VERSION :: CRATE_NAME :: TYPE_NAME)[0..8]
///
/// The crate name is included to reduce collisions between types with the
/// same name defined in different crates. Correctness across breaking
/// changes is delegated to SemVer — if the exception type changes in a
/// wire-incompatible way, the crate's major version must bump, which
/// changes the selector.
///
/// Note: structurally different types with the same name, crate, and major
/// version will intentionally share a selector. This is a tradeoff of
/// stability over collision resistance, acceptable given the u64 space.
pub fn exception_selector(
    version_str: &str,
    crate_name: &str,
    type_name: &str,
) -> u64 {
    let major_version = semver::major_version_from_string(version_str);

    let mut hasher = sha2::Sha256::new();
    hasher.update(major_version.as_bytes());
    hasher.update(b"::");
    hasher.update(crate_name.as_bytes());
    hasher.update(b"::");
    hasher.update(type_name.as_bytes());
    let result = hasher.finalize();

    u64::from_be_bytes(result[..8].try_into().unwrap())
}

/// Derive a standard event signature for a contract event type.
///
/// Event signature = sha256(MAJOR_VERSION :: CRATE_NAME :: TYPE_NAME)
///
/// Uses the same hashing scheme as `exception_selector` but returns the
/// full 32-byte hash instead of truncating to u64.
pub fn event_signature(
    version_str: &str,
    crate_name: &str,
    type_name: &str,
) -> [u8; 32] {
    let major_version = semver::major_version_from_string(version_str);

    let mut hasher = sha2::Sha256::new();
    hasher.update(major_version.as_bytes());
    hasher.update(b"::");
    hasher.update(crate_name.as_bytes());
    hasher.update(b"::");
    hasher.update(type_name.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests;
