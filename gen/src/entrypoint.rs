//! Construct and manipulate contract entrypoint IDs

use groundhog_contract_util as contract_util;
use groundhog_xdr::contract::ContractEntrypointID;
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::str::FromStr;

pub fn contract_entrypoint_id(
    interface_name: Option<&str>,
    version_str: &str,
    method: &syn::Signature,
) -> ContractEntrypointID {
    let method_signature =
        contract_util::NormalizedMethodSignature::new_from_signature(method);

    let id = contract_util::contract_entrypoint_id(
        interface_name,
        version_str,
        &method_signature,
    );

    println!("{} {:?}", hex_encode(&id), method_signature);

    id
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

use core::fmt;

pub struct Hex<'a>(&'a [u8]);

impl<'a> fmt::Display for Hex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

pub fn hex_encode<'a, B: AsRef<[u8]>>(b: &'a B) -> Hex<'a> {
    Hex(b.as_ref())
}
