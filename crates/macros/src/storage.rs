use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;
use syn::Result;
use syn::spanned::Spanned;

/// Generate trait implementations allowing a struct
/// containing various `storage` types (e.g. `Map`)
/// to be loaded.
///
/// A field's storage slot defaults to its zero-based declaration position
/// (field `i` maps to `child_index(i)`), but can be pinned explicitly with
/// `#[slot(N)]`. **Reordering struct fields remaps any positionally-assigned
/// slots** — existing on-chain state stored under the old field order will not
/// follow the renamed positions; pin slots with `#[slot(N)]` to keep them
/// stable across reorderings. Two fields resolving to the same index (whether
/// via duplicate `#[slot(N)]` or a `#[slot(N)]` landing on a positional index)
/// is a compile error.
fn storage_domain_impl(input: &syn::DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // Override via #[storage_domain = "..."], else the bare ident.
    let mut domain: Option<syn::LitStr> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("storage_domain") {
            let lit = match &attr.meta {
                syn::Meta::NameValue(syn::MetaNameValue {
                    value:
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }),
                    ..
                }) => s.clone(),
                _ => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "#[storage_domain] must be `storage_domain = \"...\"`",
                    ));
                }
            };
            domain = Some(lit);
        }
    }
    let domain_lit = match domain {
        Some(s) => quote! { #s },
        None => {
            let name = ident.to_string();
            quote! { #name }
        }
    };

    Ok(quote! {
        const _: () = {
            use ::sdk::storage::StorageDomain;
            impl #impl_generics StorageDomain for #ident #ty_generics
                #where_clause
            {
                const DOMAIN: &'static str = #domain_lit;
            }
        };
    })
}

pub fn generate_impls(input: &syn::DeriveInput) -> Result<TokenStream> {
    let item = &input.data;
    let storage_impls = generate_from_data(input, item)?;
    let domain_impl = storage_domain_impl(input)?;

    let ident = &input.ident;
    let ident_lower =
        Ident::new(&ident.to_string().to_lowercase(), ident.span());
    let impl_mod = format_ident!("_{}_storage_impls", ident_lower);

    Ok(quote! {

        mod #impl_mod {
            use super::*;

            #storage_impls
            #domain_impl
        }

    })
}

fn generate_from_data(
    input: &syn::DeriveInput,
    item: &syn::Data,
) -> Result<TokenStream> {
    match item {
        syn::Data::Struct(item) => generate_from_struct(input, item),
        _ => err(input, "Can only derive Storage for structs")?,
    }
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
    let named_fields = match fields {
        syn::Fields::Named(fields_named) => &fields_named.named,
        syn::Fields::Unit => {
            let (impl_generics, ty_generics, where_clause) =
                generics.split_for_impl();
            return Ok(quote! {
                const _: () = {
                    use ::sdk::storage::*;

                    impl #impl_generics From<Keypath> for #ident #ty_generics #where_clause {
                        fn from(_keypath: Keypath) -> Self {
                            Self
                        }
                    }
                };
            });
        }
        _ => return err(fields, "Can only handle named or unit structs"),
    };

    // Clone generics so we can add field-type bounds
    let mut generics = generics.clone();

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();
    let mut resolved: Vec<(u64, proc_macro2::Span)> = Vec::new();

    for (i, field) in named_fields.iter().enumerate() {
        let field_name = &field.ident;
        let field_type = &field.ty;
        field_names.extend(quote! { #field_name, });

        // #[slot(N)] override, else declaration position.
        let mut idx: u64 = i as u64;
        let mut slot_attr_span: Option<proc_macro2::Span> = None;
        for attr in &field.attrs {
            if attr.path().is_ident("slot") {
                if let Some(prev) = slot_attr_span {
                    let mut err = syn::Error::new(
                        attr.span(),
                        "duplicate #[slot] attribute on the same field",
                    );
                    err.combine(syn::Error::new(prev, "first #[slot] here"));
                    return Err(err);
                }
                let lit: syn::LitInt = attr.parse_args().map_err(|e| {
                    syn::Error::new(
                        attr.span(),
                        format!("#[slot(N)] expects an integer: {e}"),
                    )
                })?;
                idx = lit.base10_parse::<u64>()?;
                slot_attr_span = Some(attr.span());
            }
        }

        // Duplicate-index detection.
        if let Some((_, prev)) = resolved.iter().find(|(r, _)| *r == idx) {
            let mut err = syn::Error::new(
                field.span(),
                format!(
                    "storage slot {idx} is assigned to more than one field"
                ),
            );
            err.combine(syn::Error::new(*prev, "previously assigned here"));
            return Err(err);
        }
        resolved.push((idx, field.span()));

        field_initializers.extend(quote! {
            let #field_name = keypath.child_index(#idx).into();
        });

        // Add `FieldType: From<Keypath>` bound
        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote!(#field_type: From<Keypath>));
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        const _: () = {
            use ::sdk::storage::*;

            impl #impl_generics From<Keypath> for #ident #ty_generics
                #where_clause
            {
                fn from(keypath: Keypath) -> Self {
                    #field_initializers

                    Self {
                        #field_names
                    }
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
