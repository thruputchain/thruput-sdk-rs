//! Construct and manipulate contract entrypoint IDs

use groundhog_contract_util::EntrypointSignature;
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
