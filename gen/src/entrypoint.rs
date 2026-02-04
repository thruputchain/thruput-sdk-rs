//! Construct and manipulate contract entrypoint IDs

use groundhog_xdr::contract::ContractEntrypointID;
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::str::FromStr;

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

/// Build a method signature string for hashing.
///
/// Format: `method_name(type1,type2,...)`
fn method_signature(method: &syn::Signature) -> String {
    let method_name = method.ident.to_string();

    let arg_types: Vec<String> = method
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => {
                Some(type_to_signature_string(&pat_type.ty))
            }
        })
        .collect();

    format!("{}({})", method_name, arg_types.join(","))
}

/// Convert a type to a normalized string for signature hashing.
fn type_to_signature_string(ty: &syn::Type) -> String {
    use quote::ToTokens;
    let tokens = ty.to_token_stream();
    // Normalize whitespace
    tokens
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &syn::Signature,
) -> ContractEntrypointID {
    use sha2::Digest;

    let method_name = method.ident.to_string();
    let trait_name = interface_name.unwrap_or(&method_name);
    let major_version = cargo_major_version(version_str);
    let method_sig = method_signature(method);

    // Entrypoint ID = sha256(TRAIT_NAME || MAJOR_VERSION || METHOD_SIGNATURE)[0..8]
    let mut hasher = sha2::Sha256::new();
    hasher.update(trait_name.as_bytes());
    hasher.update(major_version.as_bytes());
    hasher.update(method_sig.as_bytes());
    let result = hasher.finalize();

    let entrypoint_id: [u8; 8] = result[0..8].try_into().unwrap();
    ContractEntrypointID::from(entrypoint_id)
}

pub fn entrypoint_export_name(id: ContractEntrypointID) -> String {
    format!("{:016X}", u64::from_be_bytes(*id))
}

pub fn entrypoint_literal(entrypoint: ContractEntrypointID) -> TokenStream {
    let name = entrypoint_export_name(entrypoint);
    let lit = Literal::from_str(&format!("0x{name}")).unwrap();

    quote!(
        ::sdk::contract::ContractEntrypointID::from_u64(#lit)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_major_version_major_nonzero() {
        assert_eq!(cargo_major_version("1.2.3"), "1");
        assert_eq!(cargo_major_version("2.0.0"), "2");
        assert_eq!(cargo_major_version("10.5.99"), "10");
    }

    #[test]
    fn test_cargo_major_version_minor_nonzero() {
        assert_eq!(cargo_major_version("0.9.2"), "0.9");
        assert_eq!(cargo_major_version("0.1.0"), "0.1");
        assert_eq!(cargo_major_version("0.15.3"), "0.15");
    }

    #[test]
    fn test_cargo_major_version_patch_only() {
        assert_eq!(cargo_major_version("0.0.2"), "0.0.2");
        assert_eq!(cargo_major_version("0.0.1"), "0.0.1");
        assert_eq!(cargo_major_version("0.0.0"), "0.0.0");
    }

    #[test]
    fn test_cargo_major_version_edge_cases() {
        // Single component
        assert_eq!(cargo_major_version("1"), "1");
        assert_eq!(cargo_major_version("0"), "0");

        // Two components
        assert_eq!(cargo_major_version("1.0"), "1");
        assert_eq!(cargo_major_version("0.1"), "0.1");
        assert_eq!(cargo_major_version("0.0"), "0.0");

        // Four components (pre-release style)
        assert_eq!(cargo_major_version("1.2.3.4"), "1");
        assert_eq!(cargo_major_version("0.1.2.3"), "0.1");
    }
}
