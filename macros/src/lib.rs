use proc_macro::TokenStream;

mod contract;
mod contract_error;
mod derive_exn;
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
        #[derive(WriteXDR, Storage)]
        #item
    }
    .into()
}

#[proc_macro_derive(Exn)]
pub fn derive_exn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_exn::derive(input)
}

#[proc_macro_attribute]
pub fn contract_error(
    _attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    contract_error::expand(item).into()
}
