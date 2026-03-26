use proc_macro::TokenStream;
use syn::DeriveInput;

pub fn derive(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let type_name = name.to_string();
    let version = std::env::var("CARGO_PKG_VERSION")
        .unwrap_or_else(|_| "0.0.0".to_string());
    let selector =
        groundhog_contract_util::error_selector(&version, &type_name);

    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // Always emit ::sdk::exn::Exn — resolves in external crates because
    // they depend on `sdk`, and inside the sdk crate itself because of
    // `extern crate self as sdk;` in sdk's lib.rs. (Same pattern as serde.)
    quote::quote! {
        impl #impl_generics ::sdk::exn::Exn for #name #ty_generics #where_clause {
            const SELECTOR: u64 = #selector;
        }
    }
    .into()
}
