//! Generate test registration

use super::entrypoint::*;
use super::{Config, ConfiguredContract, ContractSrc};
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::parse::Result;

pub fn generate(item: &syn::ItemImpl, config: &Config) -> Result<TokenStream> {
    let test_setup_mod_ident = config.test_setup_mod_ident();
    let exports_mod_ident = config.exports_mod_ident();
    let mut registrations = TokenStream::new();

    let contract = ConfiguredContract::new(config.clone(), item);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_ref().map(|s| s.as_str());

    for (index, method) in contract.methods_sorted().into_iter().enumerate() {
        registrations.extend(generate_method(index, iface_name, method)?);
    }

    let mock_registrations = registrations.clone();

    Ok(quote! {

        pub mod #test_setup_mod_ident {
            use super::#exports_mod_ident::*;
            use std::rc::Rc;

            #[allow(dead_code)]
            pub fn register(env : &Rc<::sdk::TestEnvironment>) -> ::sdk::types::Contract {
                let mut contract = env.add_contract();
                register_mock(&mut contract);
                contract.as_contract()
            }

            pub fn register_mock(contract: &mut ::sdk::MockContract) {
                #mock_registrations
            }
        }

    })
}

fn generate_method(
    index: usize,
    iface_name: Option<&str>,
    method: &syn::Signature,
) -> Result<TokenStream> {
    let ident = &method.ident;
    let internal_name = format_ident!("{}_internal", ident);
    let version = "0.0.0";
    let entrypoint_id =
        contract_entrypoint_id(iface_name, version, ident, index);
    let entrypoint = entrypoint_literal(entrypoint_id);

    Ok(quote! {
        contract.register_method(#entrypoint, #internal_name);
    })
}
