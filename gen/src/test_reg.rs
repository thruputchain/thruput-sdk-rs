//! Generate test registration

use super::entrypoint::*;
use super::{Config, ConfiguredContract, ContractSrc, Method};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

/// Generate test registration code.
///
/// For trait impls: generates `impl ::sdk::TestSetup<dyn Trait> for Type`.
/// For bare impls: generates `mod test_setup { ... }` (unchanged behavior).
pub fn generate(
    item: &syn::ItemImpl,
    config: &Config,
    trait_name: Option<&str>,
) -> Result<TokenStream> {
    if item.trait_.is_some() {
        // Error if user specified test_setup on a trait impl
        if config.test_setup.is_some() {
            return Err(syn::Error::new_spanned(
                item,
                "test_setup = \"...\" is not supported on trait impls. \
                 The #[contract] macro now generates `impl TestSetup<dyn Trait>` \
                 automatically. Use `MockContract::with_impl::<Contract, dyn Trait>()` \
                 in tests instead.",
            ));
        }
        generate_trait_impl(item, config, trait_name)
    } else {
        generate_module(item, config)
    }
}

/// Generate `impl TestSetup<dyn Trait> for Type` for trait impls.
fn generate_trait_impl(
    item: &syn::ItemImpl,
    config: &Config,
    trait_name: Option<&str>,
) -> Result<TokenStream> {
    let exports_mod_ident = config.exports_mod_ident_for_trait(trait_name);
    let mut registrations = TokenStream::new();

    let contract = ConfiguredContract::new(config.clone(), item);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    for method in contract.methods_sorted() {
        registrations.extend(generate_method_qualified(
            &contract,
            iface_name,
            &iface_vers,
            &method,
            &exports_mod_ident,
        )?);
    }

    let trait_path = item
        .trait_
        .as_ref()
        .map(|(_, path, _)| path)
        .expect("checked above: item.trait_.is_some()");
    let self_ty = &item.self_ty;

    Ok(quote! {
        impl ::sdk::TestSetup<dyn #trait_path> for #self_ty {
            fn register_mock(contract: &mut ::sdk::MockContract) {
                #registrations
            }
        }
    })
}

/// Generate `mod test_setup { ... }` for bare impls (no trait).
fn generate_module(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let test_setup_mod_ident = config.test_setup_mod_ident();
    let exports_mod_ident = config.exports_mod_ident();
    let mut registrations = TokenStream::new();

    let contract = ConfiguredContract::new(config.clone(), item);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    for method in contract.methods_sorted() {
        registrations.extend(generate_method_unqualified(
            &contract,
            iface_name,
            &iface_vers,
            &method,
        )?);
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

/// Generate a registration call with module-qualified names (for trait impl form).
fn generate_method_qualified(
    src: &impl ContractSrc,
    iface_name: Option<&str>,
    iface_vers: &str,
    method: &impl Method,
    exports_mod: &syn::Ident,
) -> Result<TokenStream> {
    let sig = method.sig();
    let ident = &sig.ident;
    let internal_name = src.make_internal_name(ident);
    let entrypoint_id = contract_entrypoint_id(iface_name, iface_vers, sig);
    let entrypoint = entrypoint_literal(entrypoint_id);

    Ok(quote! {
        contract.register_method(#entrypoint, #exports_mod::#internal_name);
    })
}

/// Generate a registration call with unqualified names (for module form).
fn generate_method_unqualified(
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
