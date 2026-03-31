use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;
use syn::Result;
use syn::spanned::Spanned;

/// Generate trait implementations allowing a struct
/// containing various `storage` types (e.g. `Map`)
/// to be loaded.
pub fn generate_impls(input: &syn::DeriveInput) -> Result<TokenStream> {
    let item = &input.data;
    let storage_impls = generate_from_data(input, item)?;

    let ident = &input.ident;
    let ident_lower =
        Ident::new(&ident.to_string().to_lowercase(), ident.span());
    let impl_mod = format_ident!("_{}_storage_impls", ident_lower);

    Ok(quote! {

        mod #impl_mod {
            use super::*;

            #storage_impls
        }

    })
}

fn generate_from_data(
    input: &syn::DeriveInput,
    item: &syn::Data,
) -> Result<TokenStream> {
    let mut output = TokenStream::new();

    output.extend(match item {
        syn::Data::Struct(item) => generate_from_struct(input, item),
        _ => err(input, "Can only derive Storage for structs")?,
    });

    Ok(output)
}

fn generate_from_struct(
    input: &syn::DeriveInput,
    item: &syn::DataStruct,
) -> Result<TokenStream> {
    let ident = &input.ident;
    let generics = &input.generics;
    let fields = &item.fields;

    impl_from_keypath(ident, generics, fields)
}

fn impl_from_keypath(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let named_fields = match fields {
        syn::Fields::Named(fields_named) => &fields_named.named,
        syn::Fields::Unit => {
            // Unit struct: From<Keypath> ignores the keypath
            return Ok(quote! {
                const _: () = {
                    use sdk::storage::*;

                    impl<#generic_params> From<Keypath> for #ty {
                        fn from(_keypath: Keypath) -> Self {
                            Self
                        }
                    }
                };
            });
        }
        _ => return err(fields, "Can only handle named or unit structs"),
    };

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();
    let n_fields = named_fields.len();
    for (i, field) in named_fields.iter().enumerate() {
        let field_name = &field.ident;
        field_names.extend(quote! { #field_name, });

        field_initializers.extend(quote! {
            let #field_name = keypath.clone().into();
        });

        if (i + 1) < n_fields {
            field_initializers.extend(quote! {
                keypath.seq_next();
            });
        }
    }

    Ok(quote! {
        const _: () = {
            use sdk::storage::*;

            impl<#generic_params> From<Keypath> for #ty {
                fn from(keypath: Keypath) -> Self {
                    let mut keypath = keypath;

                    keypath.seq_begin();
                    #field_initializers
                    // keypath.seq_end(); // unneeded

                    Self {
                        #field_names
                    }
                }
            }
        };
    })
}

fn generic_type_name(
    ident: &syn::Ident,
    generics: &syn::Generics,
) -> TokenStream {
    let generic_params = &generics.params;

    quote!(#ident #generic_params)
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(syn::Error::new(spanned.span(), msg.as_ref()))
}
