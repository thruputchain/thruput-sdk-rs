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
    let domain_lit = extract_domain_lit(input)?;

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

/// Extract the domain string literal from `#[storage_domain = "..."]` or
/// default to the struct's bare identifier name.
fn extract_domain_lit(
    input: &syn::DeriveInput,
) -> Result<proc_macro2::TokenStream> {
    let ident = &input.ident;
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
    Ok(match domain {
        Some(s) => quote! { #s },
        None => {
            let name = ident.to_string();
            quote! { #name }
        }
    })
}

pub fn generate_impls(input: &syn::DeriveInput) -> Result<TokenStream> {
    let item = &input.data;
    let storage_impls = generate_from_data(input, item)?;
    let domain_impl = storage_domain_impl(input)?;
    let const_root_impl = const_root_state_impl(input)?;

    let ident = &input.ident;
    let ident_lower =
        Ident::new(&ident.to_string().to_lowercase(), ident.span());
    let impl_mod = format_ident!("_{}_storage_impls", ident_lower);

    Ok(quote! {

        mod #impl_mod {
            use super::*;

            #storage_impls
            #domain_impl
            #const_root_impl
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

/// Resolve storage slot indices for the named fields of a struct.
///
/// Returns `Vec<(&Field, u32)>` pairing each field with its resolved slot
/// index. Errors hard on any malformed `#[slot]` attribute or duplicate index.
/// This is the single authoritative parser; both `impl_from_keypath` (runtime
/// `From<Keypath>` emission) and `const_root_state_impl` (compile-time const
/// path emission) delegate here so the two emitters can never diverge.
fn resolve_field_slots(
    named_fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> Result<Vec<(&syn::Field, u32)>> {
    let mut resolved: Vec<(&syn::Field, u32, proc_macro2::Span)> = Vec::new();

    for (i, field) in named_fields.iter().enumerate() {
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
        if let Some((_, _, prev)) = resolved.iter().find(|(_, r, _)| *r == idx)
        {
            let mut err = syn::Error::new(
                field.span(),
                format!(
                    "storage slot {idx} is assigned to more than one field"
                ),
            );
            err.combine(syn::Error::new(*prev, "previously assigned here"));
            return Err(err);
        }
        resolved.push((field, idx, field.span()));
    }

    Ok(resolved
        .into_iter()
        .map(|(field, idx, _)| (field, idx))
        .collect())
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

    // Resolve slot indices once, hard-erroring on any malformed #[slot] or
    // duplicate. const_root_state_impl reuses the same resolved list.
    let field_slots = resolve_field_slots(named_fields)?;

    // Clone generics so we can add field-type bounds
    let mut generics = generics.clone();

    let mut field_names = TokenStream::new();
    let mut field_initializers = TokenStream::new();

    for (field, idx) in &field_slots {
        let field_name = &field.ident;
        let field_type = &field.ty;
        field_names.extend(quote! { #field_name, });

        field_initializers.extend(quote! {
            let #field_name = {
                // The frame bytes are concrete at expansion time (the index
                // is a literal), so the inner const is legal even under
                // generic impls — it captures no generic params.
                const __SLOT_FRAME: ::sdk::storage::FramedComponent =
                    ::sdk::storage::FramedComponent::from_u32(#idx);
                keypath.child_framed(&__SLOT_FRAME).into()
            };
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

/// Generate `ContractState` and `FromConstPath` impls for the storage struct.
///
/// For each named field at slot index `idx`, emits a compile-time
/// `DOMAIN_FRAME ‖ FIELD_FRAME(idx)` path and constructs the field via
/// `<FieldType as FromConstPath>::__sdk_from_const_path(&PATH)`.
///
/// Slot indices come from `resolve_field_slots` — the same hard-erroring
/// parser used by `impl_from_keypath` — so const and runtime key derivation
/// are always sourced from the same indices.
///
/// Wire-safety: the byte assembly mirrors `test_keycache::keycache_const_and_owned_agree`
/// exactly — `frame_domain` for the domain frame, then the `ConstWriter`
/// pattern for the field frame, concatenated via `ConstWriter::write_opaque_fixed`.
/// If the drift tests in `keypath.rs` pass, this output is wire-correct.
fn const_root_state_impl(input: &syn::DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let domain_lit = extract_domain_lit(input)?;

    match &input.data {
        syn::Data::Struct(item) => {
            let fields = &item.fields;
            match fields {
                syn::Fields::Unit => {
                    // Unit struct: const_root() is trivial.
                    let generics = input.generics.clone();
                    let (impl_generics, ty_generics, where_clause) =
                        generics.split_for_impl();
                    Ok(quote! {
                        const _: () = {
                            use ::sdk::storage::*;

                            impl #impl_generics ::sdk::storage::ContractState
                                for #ident #ty_generics #where_clause
                            {
                                const ROOT_PATH: &'static [u8] = {
                                    // Sizes are concrete at expansion time (the domain
                                    // is a literal), so the inner consts are legal
                                    // even under generic impls — they capture no
                                    // generic params.
                                    const __N: usize =
                                        ::sdk::storage::framed_domain_len(#domain_lit);
                                    const __BYTES: [u8; __N] =
                                        ::sdk::storage::frame_domain::<__N>(#domain_lit);
                                    &__BYTES
                                };

                                #[inline]
                                fn const_root() -> Self {
                                    Self
                                }
                            }

                            impl #impl_generics ::sdk::storage::FromConstPath
                                for #ident #ty_generics #where_clause
                            {
                                #[inline]
                                fn __sdk_from_const_path(
                                    _path: &'static [u8],
                                ) -> Self {
                                    Self
                                }
                            }
                        };
                    })
                }
                syn::Fields::Named(fields_named) => {
                    let named_fields = &fields_named.named;

                    // Single authoritative slot resolution: same hard-erroring
                    // parser as impl_from_keypath, so const and runtime indices
                    // can never diverge.
                    let field_slots = resolve_field_slots(named_fields)?;

                    // Build the per-field const declarations and the
                    // const_root() body.
                    //
                    // Names like __N_F_0, __PATH_0 are indexed by field
                    // position `i` (not slot index) to guarantee uniqueness
                    // even when two fields share a slot index — which would
                    // be caught as an error by resolve_field_slots before this
                    // runs.
                    let mut path_consts = TokenStream::new();
                    let mut field_inits = TokenStream::new();
                    let mut field_names_ts = TokenStream::new();

                    // ContractState where-clause bounds:
                    // FieldType: FromConstPath for each field
                    let mut generics: syn::Generics = input.generics.clone();
                    // Also need StorageDomain supertrait satisfied —
                    // the StorageDomain impl is generated separately; just
                    // add the FromConstPath bounds here.

                    for (i, (field, idx)) in field_slots.iter().enumerate() {
                        let field_type = &field.ty;
                        let field_name = &field.ident;
                        let n_f_name = format_ident!("__N_F_{}", i);
                        let n_total_name = format_ident!("__N_TOTAL_{}", i);
                        let path_name = format_ident!("__PATH_{}", i);

                        // Emit per-field consts and static path inside
                        // the function body (where statics are allowed).
                        // __N_D is shared across all fields (declared once
                        // before the loop output is assembled), derived from
                        // ROOT_PATH.len() so framed_domain_len is called only
                        // once total (in storage_domain_impl).
                        //
                        // WIRE SPEC: the byte layout is
                        //   DOMAIN_FRAME ‖ FIELD_FRAME(idx)
                        // mirroring `DG_F0_PATH` in test_keycache exactly.
                        path_consts.extend(quote! {
                            const #n_f_name: usize =
                                ::sdk::storage::FramedComponent::from_u32(#idx).len_bytes();
                            const #n_total_name: usize = __N_D + #n_f_name;
                            // WIRE SPEC: assemble DOMAIN_FRAME ‖ FIELD_FRAME(idx)
                            // using the same ConstWriter codec as the drift test.
                            static #path_name: [u8; #n_total_name] = {
                                let d: [u8; __N_D] =
                                    ::sdk::storage::frame_domain::<__N_D>(#domain_lit);
                                let f: [u8; #n_f_name] = {
                                    let mut probe =
                                        ::sdk::__xdrs::cxs::ConstWriter::<5>::new();
                                    probe.write_u32(#idx);
                                    let v_len = probe.pos() as u32;
                                    let mut fw =
                                        ::sdk::__xdrs::cxs::ConstWriter::<#n_f_name>::new();
                                    fw.write_u32(v_len);
                                    fw.write_u32(#idx);
                                    fw.into_bytes()
                                };
                                let mut w =
                                    ::sdk::__xdrs::cxs::ConstWriter::<#n_total_name>::new();
                                w.write_opaque_fixed(&d);
                                w.write_opaque_fixed(&f);
                                w.into_bytes()
                            };
                        });

                        field_inits.extend(quote! {
                            let #field_name = <#field_type as ::sdk::storage::FromConstPath>
                                ::__sdk_from_const_path(&#path_name);
                        });
                        field_names_ts.extend(quote! { #field_name, });

                        // Add FromConstPath bound for this field type
                        generics.make_where_clause().predicates.push(
                            syn::parse_quote!(#field_type: ::sdk::storage::FromConstPath),
                        );
                    }

                    // We need StorageDomain as supertrait — it's generated
                    // by storage_domain_impl for the same struct, so the
                    // bound is always satisfied.
                    let (impl_generics, ty_generics, where_clause) =
                        generics.split_for_impl();

                    // Also build the FromConstPath fallback impl (for when
                    // this struct appears as a nested field): uses today's
                    // From<Keypath> with Keypath::from_bytes — one prefix
                    // copy (the v1 depth-1 rule). Its body only needs
                    // `Self: From<Keypath>`, so bound each field on
                    // From<Keypath> alone — a FromConstPath bound here would
                    // be pure over-constraint and needlessly reject structs
                    // with hand-written From<Keypath> field types.
                    let mut from_const_path_generics = input.generics.clone();
                    for (field, _) in &field_slots {
                        let field_type = &field.ty;
                        from_const_path_generics
                            .make_where_clause()
                            .predicates
                            .push(syn::parse_quote!(
                                #field_type: From<::sdk::storage::Keypath>
                            ));
                    }
                    let (fcp_impl_generics, _, fcp_where_clause) =
                        from_const_path_generics.split_for_impl();

                    Ok(quote! {
                        const _: () = {
                            use ::sdk::storage::*;

                            impl #impl_generics ::sdk::storage::ContractState
                                for #ident #ty_generics #where_clause
                            {
                                const ROOT_PATH: &'static [u8] = {
                                    // Sizes are concrete at expansion time (the domain
                                    // is a literal), so the inner consts are legal
                                    // even under generic impls — they capture no
                                    // generic params.
                                    const __N: usize =
                                        ::sdk::storage::framed_domain_len(#domain_lit);
                                    const __BYTES: [u8; __N] =
                                        ::sdk::storage::frame_domain::<__N>(#domain_lit);
                                    &__BYTES
                                };

                                #[inline(never)]
                                fn const_root() -> Self {
                                    // Domain frame length — shared across all
                                    // per-field path computations. The domain literal
                                    // is concrete at expansion time; the inner const
                                    // captures no generic params so it is legal
                                    // under generic impls.
                                    const __N_D: usize =
                                        ::sdk::storage::framed_domain_len(#domain_lit);

                                    // Per-field path consts and statics:
                                    // each assembles DOMAIN_FRAME ‖ FIELD_FRAME(idx)
                                    // in a function-scoped static so it is
                                    // initialised once and reused.
                                    #path_consts

                                    // Construct every field from its const path.
                                    #field_inits

                                    Self {
                                        #field_names_ts
                                    }
                                }
                            }

                            impl #fcp_impl_generics ::sdk::storage::FromConstPath
                                for #ident #ty_generics #fcp_where_clause
                            {
                                /// v1 depth-1 rule: when this struct appears as
                                /// a nested field of another storage struct,
                                /// build ONE owned Keypath from the const prefix
                                /// and delegate to today's `From<Keypath>`.
                                fn __sdk_from_const_path(
                                    path: &'static [u8],
                                ) -> Self {
                                    Self::from(
                                        ::sdk::storage::Keypath::from_bytes(path),
                                    )
                                }
                            }
                        };
                    })
                }
                _ => err(fields, "Can only handle named or unit structs"),
            }
        }
        _ => err(input, "Can only derive Storage for structs"),
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

#[cfg(test)]
mod tests {
    use super::generate_impls;

    #[test]
    fn named_struct_contains_const_root_state_impl() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Counter {
                value: NNInt,
            }
        };
        let tokens = generate_impls(&input).unwrap().to_string();
        // ContractState impl present
        assert!(
            tokens.contains("ContractState"),
            "missing ContractState: {tokens}"
        );
        // const_root method
        assert!(
            tokens.contains("const_root"),
            "missing const_root: {tokens}"
        );
        // ROOT_PATH associated const
        assert!(tokens.contains("ROOT_PATH"), "missing ROOT_PATH: {tokens}");
        // calls __sdk_from_const_path for the leaf field
        assert!(
            tokens.contains("__sdk_from_const_path"),
            "missing __sdk_from_const_path: {tokens}"
        );
        // static path array present (__PATH_0)
        assert!(
            tokens.contains("__PATH_0"),
            "missing __PATH_0 static: {tokens}"
        );
        // FromConstPath fallback impl
        assert!(
            tokens.contains("FromConstPath"),
            "missing FromConstPath: {tokens}"
        );
        // NO keypath.clone() in the const-root path
        assert!(
            !tokens.contains("keypath . clone"),
            "unexpected keypath.clone() in const-root: {tokens}"
        );
    }

    #[test]
    fn unit_struct_gets_const_root_state() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct UnitState;
        };
        let tokens = generate_impls(&input).unwrap().to_string();
        assert!(tokens.contains("ContractState"), "{tokens}");
        assert!(tokens.contains("const_root"), "{tokens}");
        assert!(tokens.contains("ROOT_PATH"), "{tokens}");
    }

    #[test]
    fn multi_field_struct_emits_path_per_field() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct State {
                a: Value<u32>,
                b: NNInt,
            }
        };
        let tokens = generate_impls(&input).unwrap().to_string();
        // Expect __PATH_0 for field 'a' and __PATH_1 for field 'b'
        assert!(tokens.contains("__PATH_0"), "{tokens}");
        assert!(tokens.contains("__PATH_1"), "{tokens}");
    }

    #[test]
    fn slot_override_uses_correct_index_in_path() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct State {
                #[slot(5)]
                first: Value<u32>,
            }
        };
        let tokens = generate_impls(&input).unwrap().to_string();
        // The path for field 0 (positional) with slot override 5 should use
        // FramedComponent::from_u32(5u32) — i.e., the literal 5 appears in
        // the path const assembly.
        assert!(tokens.contains("5u32"), "{tokens}");
    }
}
