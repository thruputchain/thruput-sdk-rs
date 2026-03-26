use proc_macro::TokenStream;
use sha2::{Digest, Sha256};
use syn::{Data, DeriveInput, Fields};

pub fn derive(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let signature = canonical_signature(&input);
    let selector = compute_selector(&signature);

    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // When the derive is used inside the `sdk` crate itself, we need
    // `crate::exn::Exn`; everywhere else we need `::sdk::exn::Exn`.
    let exn_path = if is_sdk_crate() {
        quote::quote! { crate::exn::Exn }
    } else {
        quote::quote! { ::sdk::exn::Exn }
    };

    quote::quote! {
        impl #impl_generics #exn_path for #name #ty_generics #where_clause {
            const SELECTOR: u64 = #selector;
        }
    }
    .into()
}

fn is_sdk_crate() -> bool {
    // CARGO_CRATE_NAME is set to the crate being compiled. When we're
    // compiling the `sdk` library itself it is "sdk"; for integration
    // tests of the sdk package it is the test binary name.
    std::env::var("CARGO_CRATE_NAME").ok().as_deref() == Some("sdk")
}

fn canonical_signature(input: &DeriveInput) -> String {
    let name = input.ident.to_string();

    match &input.data {
        Data::Enum(data) => {
            let variants: Vec<String> = data
                .variants
                .iter()
                .map(|v| {
                    let vname = v.ident.to_string();
                    match &v.fields {
                        Fields::Named(fields) => {
                            let ftypes: Vec<String> = fields
                                .named
                                .iter()
                                .map(|f| type_to_string(&f.ty))
                                .collect();
                            format!("{}({})", vname, ftypes.join(","))
                        }
                        Fields::Unnamed(fields) => {
                            let ftypes: Vec<String> = fields
                                .unnamed
                                .iter()
                                .map(|f| type_to_string(&f.ty))
                                .collect();
                            format!("{}({})", vname, ftypes.join(","))
                        }
                        Fields::Unit => vname,
                    }
                })
                .collect();
            format!("{}::{}", name, variants.join("::"))
        }
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let ftypes: Vec<String> = fields
                    .named
                    .iter()
                    .map(|f| type_to_string(&f.ty))
                    .collect();
                format!("{}({})", name, ftypes.join(","))
            }
            Fields::Unnamed(fields) => {
                let ftypes: Vec<String> = fields
                    .unnamed
                    .iter()
                    .map(|f| type_to_string(&f.ty))
                    .collect();
                format!("{}({})", name, ftypes.join(","))
            }
            Fields::Unit => name,
        },
        Data::Union(_) => {
            panic!("#[derive(Exn)] is not supported for unions")
        }
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

fn compute_selector(signature: &str) -> u64 {
    let hash = Sha256::digest(signature.as_bytes());
    u64::from_be_bytes(hash[..8].try_into().unwrap())
}
