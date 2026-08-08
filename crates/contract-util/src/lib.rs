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
/// The interface name used when a free function has no explicit
/// `interface = "..."` override.
pub const UNNAMED_INTERFACE: &str = "_unnamed_interface";

/// The normalized (interface, MAJOR version, method-signature) triple that
/// defines entrypoint identity. Single source of truth for
/// [`contract_entrypoint_id`] and [`entrypoint_link_name`]: both consume this
/// triple, so link-name equality coincides with ID equality by construction.
fn entrypoint_id_preimage<'a>(
    interface_name: Option<&'a str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> (&'a str, String, String) {
    (
        interface_name.unwrap_or(UNNAMED_INTERFACE),
        semver::major_version_from_string(version_str),
        method.get_signature_string(),
    )
}

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> ContractEntrypointID {
    let (interface_name, major_version, method_sig) =
        entrypoint_id_preimage(interface_name, version_str, method);

    // Entrypoint ID = sha256(INTERFACE_NAME :: MAJOR_VERSION :: METHOD_SIGNATURE)[0..8]
    //
    // The `::` separators are required for the same reason as in
    // `exception_selector`/`event_signature`: without them, distinct field
    // triples can concatenate to identical bytes (e.g. interface "Foo1" +
    // major "2" + method "x" vs "Foo" + "12" + "x", both "Foo12x"), colliding
    // to the same id and silently misrouting a call.
    let mut hasher = sha2::Sha256::new();
    hasher.update(interface_name.as_bytes());
    hasher.update(b"::");
    hasher.update(major_version.as_bytes());
    hasher.update(b"::");
    hasher.update(method_sig.as_bytes());
    let result = hasher.finalize();

    let entrypoint_id: [u8; 8] = result[0..8].try_into().unwrap();
    ContractEntrypointID::from(entrypoint_id)
}

/// The wasm export-section name of the entrypoint for the given
/// (interface, version, method) triple: [`contract_entrypoint_id`] rendered
/// through [`entrypoint_export_name`]. Use this instead of composing the two
/// by hand — when the ID preimage changes (e.g. the signature-checksum TODO
/// above), every caller stays in sync.
pub fn entrypoint_export_name_for(
    interface_name: Option<&str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> String {
    entrypoint_export_name(contract_entrypoint_id(
        interface_name,
        version_str,
        method,
    ))
}

pub fn entrypoint_export_name(id: ContractEntrypointID) -> String {
    format!("{:016X}", u64::from_be_bytes(*id))
}

/// Human-readable symbol name carrying the entrypoint-ID *preimage*, used as
/// the rv32 wrapper's `export_name` so a duplicate entrypoint ID fails the
/// build with an error naming the interface and method (rather than a bare
/// 16-hex ID). Built from the same (interface, MAJOR version, method
/// signature) triple as [`contract_entrypoint_id`], so link-name equality
/// coincides exactly with entrypoint-ID equality. Symbol names never reach
/// the lowered rv32 image (they live in non-allocatable ELF sections), so
/// readability costs nothing in the deployed artifact.
pub fn entrypoint_link_name(
    interface_name: Option<&str>,
    version_str: &str,
    method: &NormalizedMethodSignature,
) -> String {
    let (interface_name, major_version, method_sig) =
        entrypoint_id_preimage(interface_name, version_str, method);
    format!(
        "__groundhog_entrypoint::{interface_name}::{major_version}::{method_sig}"
    )
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
