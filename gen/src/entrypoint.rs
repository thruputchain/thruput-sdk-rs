//! Construct and manipulate contract entrypoint IDs

use groundhog_xdr::contract::{
    ContractEntrypointID, ContractInterfaceIDPreimage,
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::str::FromStr;
use xdrs::serialize_txs;

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method_ident: &syn::Ident,
    method_index: usize,
) -> ContractEntrypointID {
    use sha2::Digest;

    let method_name = method_ident.to_string();
    let iface_name = interface_name.unwrap_or(&method_name);
    let method_index: u16 = method_index
        .try_into()
        .expect("requested method index doesn't fit into u16");

    // println!("METHOD if:<{}> v:<{}> ix:<{}>", iface_name, version_str, method_index);

    let preimage = ContractInterfaceIDPreimage {
        name: iface_name.as_bytes().try_into().unwrap(),
        vers: version_str.as_bytes().try_into().unwrap(),
    };

    let mut hasher = sha2::Sha256::new();
    let preimage_bytes = serialize_txs::encode(&preimage).unwrap();
    hasher.update(preimage_bytes);
    let result = hasher.finalize();

    let interface_id: &[u8; 6] = &result[0..6].try_into().unwrap();
    let method_id: [u8; 2] = method_index.to_be_bytes().into();

    let mut entrypoint_id: [u8; 8] = [0; 8];
    entrypoint_id[0..6].copy_from_slice(interface_id);
    entrypoint_id[6..8].copy_from_slice(&method_id);

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
