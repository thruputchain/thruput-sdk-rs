use proc_macro::TokenStream;
use sha2::{Digest, Sha256};

pub fn storage_key_static(input: syn::LitStr) -> TokenStream {
    let hash = Sha256::digest(input.value());

    TokenStream::from(quote::quote! {
        ::thruput_sdk::types::StorageKey::new([#(#hash),*])
    })
}

pub fn sha256_static(input: syn::LitStr) -> TokenStream {
    let hash = Sha256::digest(input.value());

    TokenStream::from(quote::quote! {
        ::thruput_sdk::types::SHA256Hash::new([#(#hash),*])
    })
}

pub fn sha256_ident(input: syn::Ident) -> TokenStream {
    let string = input.to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote::quote! {
        ::thruput_sdk::types::SHA256Hash::new([#(#hash),*])
    })
}

pub fn sha256_array_ident(input: syn::Ident) -> TokenStream {
    let string = input.to_string();
    let hash = Sha256::digest(string);

    TokenStream::from(quote::quote! {
        [#(#hash),*]
    })
}

pub fn sha256_array(input: syn::Lit) -> TokenStream {
    let value = match lit_bytes(&input) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let hash = Sha256::digest(value);

    TokenStream::from(quote::quote! {
        [#(#hash),*]
    })
}

fn lit_bytes(lit: &syn::Lit) -> Result<Vec<u8>, TokenStream> {
    use syn::*;

    match lit {
        Lit::Str(lit_str) => Ok(lit_str.value().as_bytes().into()),
        Lit::ByteStr(lit_bstr) => Ok(lit_bstr.value()),
        _ => Err(syn::Error::new(
            lit.span(),
            "Can't convert literal to bytestring",
        )
        .into_compile_error()
        .into()),
    }
}
