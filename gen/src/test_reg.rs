//! Generate test registration

use super::entrypoint::*;
use super::{Config, ConfiguredContract, ContractSrc, Method};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

pub fn generate(item: &syn::ItemImpl, config: &Config) -> Result<TokenStream> {
    let test_setup_mod_ident = config.test_setup_mod_ident();
    let exports_mod_ident = config.exports_mod_ident();
    let mut registrations = TokenStream::new();

    let contract = ConfiguredContract::new(config.clone(), item);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    for method in contract.methods_sorted() {
        registrations.extend(generate_method(
            &contract,
            iface_name,
            &iface_vers,
            &method,
        )?);
    }

    let mock_registrations = registrations.clone();

    let impl_test_register = if let Some(impl_ty) = &contract.self_ty() {
        quote! {
            impl ::sdk::TestRegister for #impl_ty {
                fn register(env: &::sdk::TestEnvironmentHandle) -> ::sdk::Contract {
                    #test_setup_mod_ident::register(env)
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {

        // TODO: deprecate use of this "test_setup" module in favor of using the impl below
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

        #impl_test_register
    })
}

fn generate_method(
    src: &impl ContractSrc,
    iface_name: Option<&str>,
    iface_vers: &str,
    method: &impl Method,
) -> Result<TokenStream> {
    let sig = method.sig();
    let ident = &sig.ident;
    let internal_name = src.make_internal_name(ident);
    let entrypoint_id = contract_entrypoint_id(iface_name, iface_vers, sig);
    let entrypoint = entrypoint_literal(entrypoint_id);

    Ok(quote! {
        contract.register_method(#entrypoint, #internal_name);
    })
}
