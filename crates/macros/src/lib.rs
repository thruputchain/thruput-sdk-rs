//! Generate interfacing materials from a trait or impl that has
//! been annotated with the `contract` macro.  This crate produces:
//!
//! - Client-stub implementations for making contract calls
//! - Guest method stubs and external symbols for calling into the contract
//! - Test-registration code

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;

mod common;
mod contract;
mod contract_exception;
mod contract_type;
mod derive_contract_type_spec;
mod derive_event;
mod derive_exn;
mod static_hash;

/// Early check that all enum variants have explicit discriminants.
/// Called by attribute macros before emitting derives, so the user
/// sees one error instead of cascading failures from multiple derives.
fn check_enum_discriminants(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<()> {
    for variant in variants {
        if variant.discriminant.is_none() {
            return Err(syn::Error::new_spanned(
                variant,
                "enum variants must use explicit discriminants for contract specs",
            ));
        }
    }
    Ok(())
}

/// Shared helper: generate `impl ContractTypeName for #ident`.
/// Used by both `#[contract_type]` and `#[contract_exception]`.
fn contract_type_name_impl(
    ident: &syn::Ident,
    name: Option<&str>,
) -> proc_macro2::TokenStream {
    let name_value = match name {
        Some(custom) => quote! { #custom },
        None => quote! { stringify!(#ident) },
    };
    quote! {
        impl ::sdk::__contract_spec::ContractTypeName for #ident {
            const NAME: &'static str = #name_value;
        }
    }
}

#[cfg(feature = "storage")]
mod deletable;
#[cfg(feature = "storage")]
mod storage;

#[proc_macro]
pub fn static_key(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);

    static_hash::storage_key_static(input)
}

#[proc_macro]
pub fn sha256_static(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);

    static_hash::sha256_static(input)
}

#[proc_macro]
pub fn sha256_ident(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Ident);

    static_hash::sha256_ident(input)
}

#[proc_macro]
pub fn sha256_array_ident(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Ident);

    static_hash::sha256_array_ident(input)
}

#[proc_macro]
pub fn hash_to_array(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Lit);

    static_hash::sha256_array(input)
}

#[proc_macro_attribute]
pub fn contract(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    // Parse the item tokens into syntax defined in `syn`
    let item = syn::parse_macro_input!(item_tokens as syn::Item);

    // Apply `#[contract]` to its target item
    contract::apply(attr_args, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(feature = "storage")]
#[proc_macro_derive(Storage)]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    storage::generate_impls(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(feature = "storage")]
#[proc_macro_attribute]
pub fn storage(
    _attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = {
        let item_tokens = item_tokens.clone();
        syn::parse_macro_input!(item_tokens as syn::Item)
    };

    quote::quote! {
        #[derive(Storage)]
        #item
    }
    .into()
}

#[cfg(feature = "storage")]
#[proc_macro_derive(Deletable)]
pub fn derive_deletable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    deletable::generate_impls(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// `#[exn]` or `#[exn(version = "1")]` — emits `impl Exn` with a
/// computed selector. Use this when you handle your own derives.
/// For the full convenience macro, use `#[contract_exception]` instead.
#[proc_macro_attribute]
pub fn exn(attr_args: TokenStream, item_tokens: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    match derive_exn::parse_args(attr_args) {
        Ok(args) => derive_exn::expand(args, item),
        Err(err) => err,
    }
}

/// `#[contract_exception]` or `#[contract_exception(version = "1", name = "...")]` —
/// full convenience macro: derives + `#[repr(i32)]` + `impl Exn` +
/// `impl ContractTypeName`.
#[proc_macro_attribute]
pub fn contract_exception(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    let args = if attr_args.is_empty() {
        contract_exception::ContractExceptionArgs::default()
    } else {
        let meta_list = match darling::ast::NestedMeta::parse_meta_list(
            attr_args.into(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return TokenStream::from(
                    darling::Error::from(e).write_errors(),
                );
            }
        };
        match contract_exception::ContractExceptionArgs::from_list(&meta_list)
        {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
    contract_exception::expand(args, item).into()
}

#[proc_macro_derive(Event, attributes(topic, event_spec))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_event::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn contract_type(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    let args = if attr_args.is_empty() {
        contract_type::ContractTypeArgs::default()
    } else {
        let meta_list = match darling::ast::NestedMeta::parse_meta_list(
            attr_args.into(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return TokenStream::from(
                    darling::Error::from(e).write_errors(),
                );
            }
        };
        match contract_type::ContractTypeArgs::from_list(&meta_list) {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
    contract_type::expand(args, item).into()
}

#[proc_macro_derive(ContractTypeSpec, attributes(contract_type_spec))]
pub fn derive_contract_type_spec(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_contract_type_spec::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
