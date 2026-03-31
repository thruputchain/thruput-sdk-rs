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
    let generics = &input.generics;
    let generic_params = &generics.params;

    let body = match &item.fields {
        syn::Fields::Named(fields_named) => {
            let calls: Vec<TokenStream> = fields_named
                .named
                .iter()
                .map(|f| {
                    let name = &f.ident;
                    quote! { self.#name.delete_internal(flags); }
                })
                .collect();
            quote! { #(#calls)* }
        }
        syn::Fields::Unit => TokenStream::new(),
        _ => {
            return err(&item.fields, "Can only handle named or unit structs");
        }
    };

    let ty = generic_type_name(ident, generics);

    Ok(quote! {
        const _: () = {
            use sdk::storage::Deletable;
            use sdk::StorageDeltaFlags;

            impl<#generic_params> Deletable for #ty {
                fn delete_internal(&mut self, flags: StorageDeltaFlags) {
                    #body
                }
            }
        };
    })
}

fn generic_type_name(
    ident: &syn::Ident,
    generics: &syn::Generics,
) -> TokenStream {
    let (_, ty_generics, _) = generics.split_for_impl();
    quote!(#ident #ty_generics)
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(syn::Error::new(spanned.span(), msg.as_ref()))
}
