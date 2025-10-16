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
        syn::Data::Struct(ref item) => generate_from_struct(input, item),
        _ => err(input, "Can only derive Storage for structs")?,
    });

    output.extend(impl_load_via_read(input));

    Ok(output)
}

fn impl_load_via_read(input: &syn::DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let generics = &input.generics;

    Ok(quote! {
        const _: () = {
            use sdk::storage::*;
            use sdk::storage::error::Result;

            impl #generics LoadKeypath for #ident #generics {
                fn load_keypath(keypath: &Keypath) -> Result<Self> {
                    load_readable(keypath)
                }
            }
        };
    })
}

fn generate_from_struct(
    input: &syn::DeriveInput,
    item: &syn::DataStruct,
) -> Result<TokenStream> {
    let ident = &input.ident;
    let generics = &input.generics;
    let fields = &item.fields;

    let mut output = TokenStream::new();
    output.extend(impl_read_xdr_path(ident, generics, fields)?);
    output.extend(impl_default_from_keypath(ident, generics, fields)?);

    Ok(output)
}

fn impl_read_xdr_path(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let fields = if let syn::Fields::Named(ref fields_named) = fields {
        &fields_named.named
    } else {
        return err(fields, "Can only handle named struct fields");
    };

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();
    let n_fields = fields.len();
    for (i, field) in fields.iter().enumerate() {
        let field_name = &field.ident;
        field_names.extend(quote! { #field_name, });

        field_initializers.extend(quote! {
            let #field_name = ReadXDRPath::read_xdr_path(r)?;
        });

        if i + 1 < n_fields {
            field_initializers.extend(quote! {
                r.seq_next();
            });
        }
    }

    Ok(quote! {
        const _: () = {
            use sdk::serialize::ReadResult;
            use sdk::storage::*;

            impl<#generic_params> ReadXDRPath for #ty {
                fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
                where
                    R: ReaderPath<'r, Keypath>
                {
                    r.seq_begin();

                    #field_initializers

                    r.seq_end();

                    Ok(Self {
                        #field_names
                    })
                }
            }
        };
    })
}

fn impl_default_from_keypath(
    ident: &syn::Ident,
    generics: &syn::Generics,
    fields: &syn::Fields,
) -> Result<TokenStream> {
    let ty = generic_type_name(ident, generics);
    let generic_params = &generics.params;

    let fields = if let syn::Fields::Named(ref fields_named) = fields {
        &fields_named.named
    } else {
        return err(fields, "Can only handle named struct fields");
    };

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();
    let n_fields = fields.len();
    for (i, field) in fields.iter().enumerate() {
        let field_name = &field.ident;
        field_names.extend(quote! { #field_name, });

        field_initializers.extend(quote!{
            let #field_name = DefaultFromKeypath::default_from_keypath(&keypath);
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

            impl<#generic_params> DefaultFromKeypath for #ty {
                fn default_from_keypath(keypath: &Keypath) -> Self {
                    let mut keypath = keypath.clone();

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
