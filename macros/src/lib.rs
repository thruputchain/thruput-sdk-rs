use proc_macro::TokenStream;

mod contract;
mod contract_error;
mod contract_type;
mod derive_contract_type_spec;
mod derive_event;
mod derive_exn;
mod static_hash;

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
/// For the full convenience macro, use `#[contract_error]` instead.
#[proc_macro_attribute]
pub fn exn(attr_args: TokenStream, item_tokens: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    match derive_exn::parse_args(attr_args) {
        Ok(args) => derive_exn::expand(args, item),
        Err(err) => err,
    }
}

/// `#[contract_error]` or `#[contract_error(version = "1")]` — full
/// convenience macro: derives + `#[repr(i32)]` + `impl Exn`.
#[proc_macro_attribute]
pub fn contract_error(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    match derive_exn::parse_args(attr_args) {
        Ok(args) => contract_error::expand(args, item).into(),
        Err(err) => err,
    }
}

#[proc_macro_derive(Event, attributes(topic))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_event::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn contracttype(
    _attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    contract_type::expand(item).into()
}

#[proc_macro_derive(ContractTypeSpec)]
pub fn derive_contract_type_spec(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_contract_type_spec::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
