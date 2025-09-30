use proc_macro::TokenStream;

mod contract;
mod static_key;
mod storage;

#[proc_macro]
pub fn static_key(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);
    static_key::static_key_output(input)
}

/// Generate contract-related code
#[proc_macro_attribute]
pub fn contract(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    // Parse the item tokens into syntax defined in `syn`
    let item = {
        let item_tokens = item_tokens.clone();
        syn::parse_macro_input!(item_tokens as syn::Item)
    };

    contract::generate_output(attr_args, item_tokens, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Storage)]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    storage::generate_impls(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Attribute-macro `storage` simply decorates the item
/// with `#[derive(WriteXDR, Storage)]`
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
    }.into()
}
