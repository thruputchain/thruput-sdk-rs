//! Construct and manipulate contract entrypoint IDs

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::str::FromStr;
use thruput_contract_util::EntrypointSignature;
pub use thruput_contract_util::entrypoint_export_name;
use thruput_xdr::contract::ContractEntrypointID;

pub fn trait_entrypoint_ident(method: &syn::Ident) -> syn::Ident {
    quote::format_ident!("__GROUNDHOG_ENTRYPOINT_{}", method)
}

/// Emit a reference for trait impls; only interface declarations and bare
/// contracts compute IDs during macro expansion.
pub fn method_id_tokens(
    contract: &impl super::contract::ContractSrc,
    method: &syn::Signature,
) -> TokenStream {
    if let (Some(trait_path), Some(self_ty)) =
        (contract.trait_path(), contract.self_ty())
    {
        let ident = trait_entrypoint_ident(&method.ident);
        quote!(<#self_ty as #trait_path>::#ident)
    } else {
        let id = contract_entrypoint_id(
            contract.interface_name().as_deref(),
            &contract.defaulted_interface_vers(),
            method,
        );
        let id = u64::from_be_bytes(*id);
        quote!(#id)
    }
}

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &syn::Signature,
) -> ContractEntrypointID {
    EntrypointSignature::new(interface_name, version_str, method).id()
}

/// The `::thruput_sdk::InterfaceId` literal for an (interface, version)
/// pair, from [`thruput_contract_util::interface_id`].
pub fn interface_id_literal(
    interface_name: Option<&str>,
    version_str: &str,
) -> TokenStream {
    let bytes =
        thruput_contract_util::interface_id(interface_name, version_str);
    quote!([#(#bytes),*])
}

pub fn entrypoint_literal(entrypoint: ContractEntrypointID) -> TokenStream {
    let name = entrypoint_export_name(entrypoint);
    let lit = Literal::from_str(&format!("0x{name}")).unwrap();

    quote!(
        ::thruput_sdk::contract::ContractEntrypointID::from_u64(#lit)
    )
}
