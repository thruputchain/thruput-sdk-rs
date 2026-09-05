//! Construct and manipulate contract entrypoint IDs

use groundhog_contract_util::EntrypointSignature;
pub use groundhog_contract_util::entrypoint_export_name;
use groundhog_xdr::contract::ContractEntrypointID;
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::str::FromStr;

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &syn::Signature,
) -> ContractEntrypointID {
    EntrypointSignature::new(interface_name, version_str, method).id()
}

/// The `::sdk::InterfaceId` literal for an (interface, version) pair, from
/// [`groundhog_contract_util::interface_id`].
pub fn interface_id_literal(
    interface_name: Option<&str>,
    version_str: &str,
) -> TokenStream {
    let bytes =
        groundhog_contract_util::interface_id(interface_name, version_str);
    quote!(
        ::sdk::InterfaceId::new([#(#bytes),*])
    )
}

pub fn entrypoint_literal(entrypoint: ContractEntrypointID) -> TokenStream {
    let name = entrypoint_export_name(entrypoint);
    let lit = Literal::from_str(&format!("0x{name}")).unwrap();

    quote!(
        ::sdk::contract::ContractEntrypointID::from_u64(#lit)
    )
}
