//! Generate test registration

use super::config::Config;
use super::contract::{ConfiguredContract, ContractSrc, Method};
use super::entrypoint::*;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

/// For trait impls: returns a syn::ImplItem to inject into the impl block.
pub fn generate_trait_method(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<syn::ImplItem> {
    let registrations = build_registrations(item, config)?;

    Ok(syn::parse_quote! {
        #[doc(hidden)]
        fn __mock(contract: &mut ::sdk::MockContract) where Self: Sized {
            #registrations
        }
    })
}

/// For bare impls: returns a standalone impl block with __mock.
pub fn generate_bare_method(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let registrations = build_registrations(item, config)?;

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

/// Generate a crate-global registry fragment for an impl item.
pub fn generate_impl_registry_fragment(
    item: &syn::ItemImpl,
    _config: &Config,
) -> Result<TokenStream> {
    let self_ty = &item.self_ty;
    let register_path = if let Some((trait_path, _)) = &item.trait_ {
        quote! { <#self_ty as #trait_path>::__mock }
    } else {
        quote! { #self_ty::__mock }
    };

    // The registry admits one storage-bearing self type per package. An
    // impl block none of whose methods take `self` never loads that type's
    // state, so it is stateless like a free function and registers like
    // one: it may sit beside the contract's storage type (an ERC-165
    // carrier struct, say) without counting as a second contract.
    let stateless = item.items.iter().all(|impl_item| match impl_item {
        syn::ImplItem::Fn(f) => f.sig.receiver().is_none(),
        _ => true,
    });
    let self_type_key = if stateless {
        quote! { None }
    } else {
        quote! { Some(stringify!(#self_ty)) }
    };

    Ok(quote! {
        const _: () = {
            fn register(contract: &mut ::sdk::MockContract) {
                #register_path(contract);
            }

            #[::sdk::__linkme::distributed_slice(::sdk::mock_registry::CONTRACT_MOCK_FRAGMENTS)]
            #[linkme(crate = ::sdk::__linkme)]
            static FRAGMENT: ::sdk::mock_registry::ContractMockFragment =
                ::sdk::mock_registry::ContractMockFragment {
                    package_name: env!("CARGO_PKG_NAME"),
                    self_type_key: #self_type_key,
                    register,
                };
        };
    })
}

/// Generate a crate-global registry fragment for a free function item.
pub fn generate_free_function_registry_fragment(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let effective_interface_name =
        super::contract::effective_interface_name(&contract);
    let exports_mod_ident = if config.exports.is_some() {
        config.internal_exports_mod_ident(&effective_interface_name)
    } else {
        super::exports::free_function_exports_mod_ident(
            &effective_interface_name,
            &item.sig.ident,
        )
    };

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    let registration = generate_method_qualified(
        &contract,
        iface_name,
        &iface_vers,
        item,
        &exports_mod_ident,
    )?;

    Ok(quote! {
        const _: () = {
            fn register(contract: &mut ::sdk::MockContract) {
                #registration
            }

            #[::sdk::__linkme::distributed_slice(::sdk::mock_registry::CONTRACT_MOCK_FRAGMENTS)]
            #[linkme(crate = ::sdk::__linkme)]
            static FRAGMENT: ::sdk::mock_registry::ContractMockFragment =
                ::sdk::mock_registry::ContractMockFragment {
                    package_name: env!("CARGO_PKG_NAME"),
                    self_type_key: None,
                    register,
                };
        };
    })
}

/// Build the registration calls shared by both trait and bare methods.
fn build_registrations(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let effective_interface_name =
        super::contract::effective_interface_name(&contract);
    let exports_mod_ident =
        config.internal_exports_mod_ident(&effective_interface_name);
    let mut registrations = TokenStream::new();

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
