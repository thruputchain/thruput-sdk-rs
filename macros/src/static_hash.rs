use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use sha2::{Digest, Sha256};

pub fn storage_key_static(input: syn::LitStr) -> TokenStream {
    let string = input.token().to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote! {
        ::sdk::types::StorageKey::new([#(#hash),*])
    })
}

pub fn sha256_static(input: syn::LitStr) -> TokenStream {
    let array = sha256_array(input);

    quote! {
        ::sdk::types::SHA256Hash::new(#array)
    }
    .into()
}

pub fn sha256_array(input: syn::LitStr) -> TokenStream2 {
    let string = input.token().to_string();
    let hash = Sha256::digest(string);

    quote! {
        [#(#hash),*]
    }
}
