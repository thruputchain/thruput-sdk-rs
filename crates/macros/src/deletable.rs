use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;
use syn::spanned::Spanned;

/// Generate a `Deletable` impl for a struct decorated with
/// `#[derive(Deletable)]`.
pub fn generate_impls(input: &syn::DeriveInput) -> Result<TokenStream> {
    match &input.data {
        syn::Data::Struct(item) => generate_from_struct(input, item),
        _ => err(input, "Can only derive Deletable for structs"),
    }
}

fn generate_from_struct(
    input: &syn::DeriveInput,
    item: &syn::DataStruct,
) -> Result<TokenStream> {
    let ident = &input.ident;

    // Clone generics so we can add field-type bounds to the where clause
    let mut generics = input.generics.clone();

    let body = match &item.fields {
        syn::Fields::Named(fields_named) => {
            let calls: Vec<TokenStream> = fields_named
                .named
                .iter()
                .map(|f| {
                    let name = &f.ident;
                    quote! { self.#name.delete_internal(exclusive); }
                })
                .collect();

            // Add `FieldType: Deletable` bound for each field
            for field in &fields_named.named {
                let ty = &field.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(syn::parse_quote!(#ty: Deletable));
            }

            quote! { #(#calls)* }
        }
        // Nothing to delete, so the policy goes unused.
        syn::Fields::Unit => quote! { let _ = exclusive; },
        _ => {
            return err(&item.fields, "Can only handle named or unit structs");
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        const _: () = {
            use ::thruput_sdk::storage::Deletable;

            impl #impl_generics Deletable for #ident #ty_generics
                #where_clause
            {
                fn delete_internal(&mut self, exclusive: bool) {
                    #body
                }
            }
        };
    })
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(syn::Error::new(spanned.span(), msg.as_ref()))
}
