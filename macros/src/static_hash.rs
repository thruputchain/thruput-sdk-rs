use proc_macro::TokenStream;
use sha2::{Digest, Sha256};

pub fn storage_key_static(input: syn::LitStr) -> TokenStream {
    let string = input.token().to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote::quote! {
        ::sdk::types::StorageKey::new([#(#hash),*])
    })
}

pub fn sha256_static(input: syn::LitStr) -> TokenStream {
    let string = input.token().to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote::quote! {
        ::sdk::types::SHA256Hash::new([#(#hash),*])
    })
}

pub fn topic_hashed_from(input: syn::LitStr) -> TokenStream {
    let string = input.token().to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote::quote! {
        [#(#hash),*]
    })
}
