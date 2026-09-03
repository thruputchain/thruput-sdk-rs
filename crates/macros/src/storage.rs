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
/// (field `i` maps to `child(&(i as u32))`), but can be pinned explicitly with
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
                const DOMAIN: ::sdk::storage::FramedDomain = {
                    // Sizes are concrete at expansion time (the domain is a
                    // literal), so no generic_const_exprs is needed, and the
                    // inner consts are legal even under generic impls
                    // because they capture no generic params.
                    const N: usize =
                        ::sdk::storage::framed_domain_len(#domain_lit);
                    const BYTES: [u8; N] =
                        ::sdk::storage::frame_domain::<N>(#domain_lit);
                    ::sdk::storage::FramedDomain::new(&BYTES)
                };
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
    let mut resolved: Vec<(u32, proc_macro2::Span)> = Vec::new();
    let generic_idents: Vec<syn::Ident> = generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(t.ident.clone()),
            syn::GenericParam::Const(c) => Some(c.ident.clone()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect();

    for (i, field) in named_fields.iter().enumerate() {
        let field_name = &field.ident;
        let field_type = &field.ty;
        field_names.extend(quote! { #field_name, });

        // #[slot(N)] override, else declaration position. The index is a
        // `u32` so the dedup domain matches the emit domain
        // (`keypath.child(&#idx)` with `#idx: u32`); otherwise two slots that
        // are distinct as u64 but congruent mod 2^32 would pass the dedup yet
        // emit the same key. Field count is safely << 2^32 (host-side macro).
        let mut idx: u32 = i as u32;
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
                // Parse as u32: a `#[slot(N)]` with N >= 2^32 is a compile
                // error at the literal span, so the truncation-collision case
                // is inexpressible.
                idx = lit.base10_parse::<u32>().map_err(|_| {
                    syn::Error::new(
                        lit.span(),
                        "#[slot(N)] index must fit in a u32",
                    )
                })?;
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
            let #field_name = {
                // The frame bytes are concrete at expansion time (the index
                // is a literal), so the inner const is legal even under
                // generic impls — it captures no generic params.
                const __SLOT_FRAME: ::sdk::storage::FramedComponent =
                    ::sdk::storage::FramedComponent::from_u32(#idx);
                // Through the `StorageType`-bounded helper rather than
                // `.into()`: it makes a non-storage field fail `T:
                // StorageType` here, at the field, instead of failing a
                // bare `From<Keypath>` bound at the derive's span.
                ::sdk::storage::__field_from_keypath::<#field_type>(
                    keypath.child_framed(&__SLOT_FRAME),
                )
            };
        });

        // A generic field type needs its `StorageType` bound stated in the
        // where clause; a concrete one must not have it there. A
        // where-clause bound on a concrete type that cannot satisfy it is
        // *trivially* unsatisfiable, which rustc reports at the derive's
        // span with a `#![feature(trivial_bounds)]` suggestion — never the
        // fix, and it does not say which field is wrong. Left out, the
        // helper call in the initializer above raises the same obligation as
        // an ordinary one, so the error lands on the field and carries
        // `StorageType`'s `on_unimplemented`.
        if type_mentions_generic(field_type, &generic_idents) {
            generics.make_where_clause().predicates.push(
                syn::parse_quote!(#field_type: ::sdk::storage::StorageType),
            );
        }
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

            // A `#[storage]` struct is itself a storage type: it may be a
            // field of an enclosing one, or a `Map`/`Vector` element. The
            // seal lives in the SDK, so only this derive can grant it.
            impl #impl_generics ::sdk::storage::Sealed for #ident #ty_generics
                #where_clause {}
            impl #impl_generics ::sdk::storage::StorageType for #ident #ty_generics
                #where_clause {}
        };
    })
}

/// Whether `ty` mentions any of `params` — i.e. whether a bound on it is
/// resolved per-instantiation rather than trivially at the definition.
///
/// Token-level on purpose: a false positive (a path segment that merely
/// shares an identifier with a parameter) only means the field keeps the
/// where-clause bound, which is what the derive did for every field before.
fn type_mentions_generic(ty: &syn::Type, params: &[syn::Ident]) -> bool {
    if params.is_empty() {
        return false;
    }
    quote!(#ty)
        .into_iter()
        .any(|tt| tokens_mention(&tt, params))
}

fn tokens_mention(tt: &proc_macro2::TokenTree, params: &[syn::Ident]) -> bool {
    match tt {
        proc_macro2::TokenTree::Ident(id) => params.contains(id),
        proc_macro2::TokenTree::Group(g) => {
            g.stream().into_iter().any(|t| tokens_mention(&t, params))
        }
        _ => false,
    }
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(syn::Error::new(spanned.span(), msg.as_ref()))
}
