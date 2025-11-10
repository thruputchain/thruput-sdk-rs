use proc_macro::TokenStream;

mod contract;
mod static_hash;

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
pub fn topic_hashed_from(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);

    static_hash::topic_hashed_from(input)
}

/// Generate contract-related code
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

/// Attribute-macro `storage` simply decorates the item
/// with `#[derive(WriteXDR, Storage)]`
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
        #[derive(WriteXDR, Storage)]
        #item
    }
    .into()
}
