//! Generate test registration

use super::entrypoint::*;
use super::{Config, ConfiguredContract, ContractSrc, Method};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

/// Generate test registration code.
///
/// For trait impls: generates `impl ::sdk::TestSetup<dyn Trait> for Type`.
/// For bare impls: generates `impl ::sdk::TestSetup<Type> for Type`.
pub fn generate(
    item: &syn::ItemImpl,
    config: &Config,
    trait_name: Option<&str>,
) -> Result<TokenStream> {
    // Error if user specified test_setup on any impl
    if config.test_setup.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "test_setup = \"...\" is no longer supported. \
             The #[contract] macro now generates `impl TestSetup` \
             automatically. Use `env.register_mock::<Contract, Type>()` \
             or `MockContract::with_impl::<Contract, Type>()` in tests instead.",
        ));
    }
    if item.trait_.is_some() {
        generate_trait_impl(item, config, trait_name)
    } else {
        generate_bare_impl(item, config)
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

/// Generate `impl TestSetup<Type> for Type` for bare impls (no trait).
fn generate_bare_impl(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let exports_mod_ident = config.exports_mod_ident();
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

    let self_ty = &item.self_ty;

    Ok(quote! {
        impl ::sdk::TestSetup<#self_ty> for #self_ty {
            fn register_mock(contract: &mut ::sdk::MockContract) {
                #registrations
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
