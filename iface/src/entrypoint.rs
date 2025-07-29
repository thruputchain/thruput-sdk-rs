//! Construct and manipulate contract entrypoint IDs

use groundhog_xdr::contract::ContractEntrypointID;
use xdrs::serialize_txs;
use quote::quote;
use proc_macro2::TokenStream;

pub fn contract_entrypoint_id(
    trait_ident: Option<&syn::Ident>,
    version_str: Option<&str>,
    method_ident: &syn::Ident,
    method_index: usize,
) -> ContractEntrypointID {
    use sha2::Digest;

    let iface_ident = trait_ident.unwrap_or(method_ident);
    let iface_name = iface_ident.to_string();
    let version_str = version_str.unwrap_or("0.0.0");
    let method_index: u16 = method_index.try_into().expect("requested method index doesn't fit into u16");

    // println!("METHOD if:<{}> v:<{}> ix:<{}>", iface_name, version_str, method_index);

    let preimage =
        ::groundhog_xdr::contract::ContractInterfaceIDPreimage {
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
    let mut s = String::new();
    let bytes: &[u8] = id.as_ref();
    let method : u64 = u64::from_be_bytes(bytes.try_into().unwrap());
    s.push_str(&format!("{:X}", method));
    s
}

pub fn entrypoint_literal(entrypoint: ContractEntrypointID) -> TokenStream {
    let bytes: &[u8] = entrypoint.as_ref();
    quote!(
        sdk::types::ContractEntrypointID::new([#(#bytes),*])
    )
}
