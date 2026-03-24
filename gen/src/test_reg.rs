//! Generate test registration

use super::entrypoint::*;
use super::{Config, ConfiguredContract, ContractSrc, Method};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

/// For trait impls: returns a syn::ImplItem to inject into the impl block.
pub fn generate_trait_method(
    item: &syn::ItemImpl,
    config: &Config,
    trait_name: Option<&str>,
) -> Result<Option<syn::ImplItem>> {
    // Error if user specified test_setup on any impl
    if config.test_setup.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "test_setup = \"...\" is no longer supported. \
             The #[contract] macro now generates `__mock` \
             automatically. Use `<Type as Trait>::__mock` in tests instead.",
        ));
    }

    let registrations = build_registrations(item, config, trait_name)?;

    Ok(Some(syn::parse_quote! {
        #[doc(hidden)]
        fn __mock(contract: &mut ::sdk::MockContract) where Self: Sized {
            #registrations
        }
    }))
}

/// For bare impls: returns a standalone impl block with __mock.
pub fn generate_bare_method(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    // Error if user specified test_setup on any impl
    if config.test_setup.is_some() {
        return Err(syn::Error::new_spanned(
            item,
            "test_setup = \"...\" is no longer supported. \
             The #[contract] macro now generates `__mock` \
             automatically. Use `Type::__mock` in tests instead.",
        ));
    }

    let registrations = build_registrations(item, config, None)?;

    let self_ty = &item.self_ty;

    Ok(quote! {
        impl #self_ty {
            #[doc(hidden)]
            pub fn __mock(contract: &mut ::sdk::MockContract) {
                #registrations
            }
        }
    })
}

/// Build the registration calls shared by both trait and bare methods.
fn build_registrations(
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

    Ok(registrations)
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
