use proc_macro::TokenStream;

pub fn static_key_output(input: syn::LitStr, internal: bool) -> TokenStream {
    let string = input.token().to_string();
    let res = make_static_key(&string);

    if internal {
        TokenStream::from(quote::quote! {
            {
                use crate::types::byte_array::StorageKey;

                StorageKey::new([#(#res),*])
            }
        })
    } else {
        TokenStream::from(quote::quote! {
            {
                use ::sdk::types::StorageKey;

                StorageKey::new([#(#res),*])
            }
        })
    }
}

fn make_static_key(arg: &str) -> [u8; 32] {
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(arg.as_bytes());
    hasher.finalize().into()
}
