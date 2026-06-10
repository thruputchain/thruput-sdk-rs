use crate::common::contract::{
    contract_type_anchor_token, contract_type_view_tokens,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, GenericParam, Result};

/// Returns (topic_fields, payload_fields).
fn partition_fields(
    fields: &Fields,
) -> Result<(Vec<&syn::Field>, Vec<&syn::Field>)> {
    let Fields::Named(named) = fields else {
        return Err(syn::Error::new_spanned(
            fields,
            "#[derive(Event)] only supports structs with named fields",
        ));
    };

    let mut topics = Vec::new();
    let mut payload = Vec::new();

    for field in &named.named {
        if field.attrs.iter().any(|a| a.path().is_ident("topic")) {
            topics.push(field);
        } else {
            payload.push(field);
        }
    }

    Ok((topics, payload))
}

/// Replaces every lifetime in `ty` with `'static`.
///
/// Event structs may borrow their fields (e.g. `&'a ContractAddress`), but
/// the generated spec fragment is a `const` item where the struct's lifetime
/// parameters are not in scope. The spec only describes the *type shape*, so
/// the concrete lifetime is irrelevant.
fn fold_lifetimes_to_static(ty: &syn::Type) -> syn::Type {
    struct F;
    impl syn::fold::Fold for F {
        fn fold_lifetime(&mut self, lt: syn::Lifetime) -> syn::Lifetime {
            syn::Lifetime::new("'static", lt.span())
        }
    }
    syn::fold::Fold::fold_type(&mut F, ty.clone())
}

fn field_view_tokens(field: &syn::Field) -> Result<TokenStream> {
    let name = field.ident.as_ref().unwrap().to_string();
    let ty = fold_lifetimes_to_static(&field.ty);
    let ty_tokens = contract_type_view_tokens(&ty)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractFieldView {
            name: #name,
            ty: #ty_tokens,
        }
    })
}

/// Emits the raw event spec fragment plus linker anchors for the event's
/// field types, mirroring `derive_contract_type_spec::emit_raw_typedef_fragment`.
fn emit_raw_event_fragment(
    ident: &syn::Ident,
    type_name: &str,
    sig_bytes: &[u8; 32],
    topics: &[&syn::Field],
    payload: &[&syn::Field],
) -> Result<TokenStream> {
    let fragment_ident =
        format_ident!("__groundhog_spec_event_fragment_{ident}");
    let raw_len_ident =
        format_ident!("__groundhog_spec_event_raw_len_{ident}");
    let raw_fragment_ident =
        format_ident!("__groundhog_spec_raw_event_fragment_{ident}");

    let topic_views = topics
        .iter()
        .map(|field| field_view_tokens(field))
        .collect::<Result<Vec<_>>>()?;
    let payload_views = payload
        .iter()
        .map(|field| field_view_tokens(field))
        .collect::<Result<Vec<_>>>()?;

    let anchor_tokens = topics
        .iter()
        .chain(payload.iter())
        .map(|field| {
            contract_type_anchor_token(&fold_lifetimes_to_static(&field.ty))
        })
        .collect::<Result<Vec<_>>>()?;
    let anchor_count = anchor_tokens.len();
    let anchors = if anchor_count == 0 {
        quote! {}
    } else {
        // Retains the raw typedef fragments of the event's field types, so
        // `Named` references in the event def resolve during spec assembly.
        quote! {
            #[cfg(any(target_family = "wasm", target_arch = "riscv32"))]
            const _: () = {
                #[used]
                static __GROUNDHOG_EVENT_SPEC_ANCHORS: [fn() -> usize; #anchor_count] = [
                    #(#anchor_tokens),*
                ];
            };
        }
    };

    let sig_array = sig_bytes.iter().copied();

    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    // On RV32 this is an ALLOC data section, retained by `#[used]`, and the
    // groundhog-rpc lowerer excludes it from VM memory before artifact packing.
    Ok(quote! {
        #[doc(hidden)]
        pub const #fragment_ident: ::sdk::__contract_spec::SpecFragmentView<'static> =
            ::sdk::__contract_spec::SpecFragmentView::Event(
                ::sdk::__contract_spec::ContractEventDefView {
                    name: #type_name,
                    signature: [#(#sig_array),*],
                    topics: &[#(#topic_views),*],
                    fields: &[#(#payload_views),*],
                }
            );

        #[doc(hidden)]
        const #raw_len_ident: usize = #fragment_ident.const_txs_size();

        #[cfg(any(target_family = "wasm", target_arch = "riscv32"))]
        #[doc(hidden)]
        #[cfg_attr(
            any(target_family = "wasm", target_arch = "riscv32"),
            unsafe(link_section = "groundhogspecrawv0")
        )]
        // RV32 only: `#[used]` keeps the section live through ELF linking for CLI
        // extraction. On wasm the custom section is retained without it, and
        // `#[used]` there would force the fragment bytes into linear memory
        // (tripping the spec-tool leak guard).
        #[cfg_attr(target_arch = "riscv32", used)]
        static #raw_fragment_ident: [u8; #raw_len_ident] = {
            let mut w =
                ::sdk::__xdrs::txs::ConstWriter::<#raw_len_ident>::new();
            #fragment_ident.const_write_txs(&mut w);
            w.into_bytes()
        };

        #anchors
    })
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Event)] only supports structs",
        ));
    };

    let (topics, payload) = partition_fields(&data.fields)?;

    let max_topics = groundhog_xdr::event::kMaxTopics as usize;
    let total_topics = topics.len() + 1; // signature topic
    if total_topics > max_topics {
        return Err(syn::Error::new_spanned(
            input,
            format!(
                "#[derive(Event)] supports at most {} total topics (signature + {} #[topic] fields), but this event declares {} #[topic] fields",
                max_topics,
                max_topics.saturating_sub(1),
                topics.len(),
            ),
        ));
    }

    let ident = &input.ident;
    let vis = &input.vis;
    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // --- SIGNATURE const ---
    let type_name = ident.to_string();
    let version = std::env::var("CARGO_PKG_VERSION")
        .unwrap_or_else(|_| "0.0.0".to_string());
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());
    let sig_bytes = groundhog_contract_util::event_signature(
        &version,
        &crate_name,
        &type_name,
    );

    let sig_array = sig_bytes.iter().copied();

    // --- Raw spec fragment for link-section based spec extraction ---
    let raw_event_fragment = emit_raw_event_fragment(
        ident, &type_name, &sig_bytes, &topics, &payload,
    )?;

    // --- Event impl ---
    let num_topics = (topics.len() + 1) as u32; // +1 for SIGNATURE at index 0

    let topic_arms = topics.iter().enumerate().map(|(i, field)| {
        let index = (i + 1) as u32; // 0 is SIGNATURE
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            #index => ::core::option::Option::Some(
                self.#field_ident.into_topic()
            ),
        }
    });

    // --- WriteXDR impl ---
    let payload_writes = payload.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            ::sdk::WriteXDR::write_xdr(&self.#field_ident, w)?;
        }
    });

    // --- Payload struct ---
    let payload_ident = format_ident!("{}Payload", ident);

    let payload_fields: Vec<_> = payload
        .iter()
        .map(|field| {
            let vis = &field.vis;
            let ident = &field.ident;
            let ty = &field.ty;
            let attrs: Vec<_> = field
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("topic"))
                .collect();
            quote! { #(#attrs)* #vis #ident: #ty }
        })
        .collect();

    // Copy lifetime params from the event struct for the payload struct
    let lifetime_params: Vec<_> = input
        .generics
        .params
        .iter()
        .filter(|p| matches!(p, GenericParam::Lifetime(_)))
        .collect();

    let payload_generics = if lifetime_params.is_empty() {
        quote! {}
    } else {
        quote! { < #(#lifetime_params),* > }
    };

    // For the GAT assignment: map all lifetime params to '__payload
    let payload_lifetime_args: Vec<_> = input
        .generics
        .params
        .iter()
        .filter(|p| matches!(p, GenericParam::Lifetime(_)))
        .map(|_| quote! { '__payload })
        .collect();

    let payload_ty_in_gat = if payload_lifetime_args.is_empty() {
        quote! { #payload_ident }
    } else {
        quote! { #payload_ident < #(#payload_lifetime_args),* > }
    };

    Ok(quote! {
        #raw_event_fragment

        impl #impl_generics #ident #ty_generics #where_clause {
            pub const SIGNATURE: &'static ::sdk::Topic = &[#(#sig_array),*];
        }

        #[derive(::sdk::ReadXDR)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[allow(dead_code)]
        #vis struct #payload_ident #payload_generics {
            #(#payload_fields,)*
            #[allow(dead_code)]
            __phantom: ::core::marker::PhantomData<( #(&#lifetime_params ()),* )>,
        }

        impl #impl_generics ::sdk::Event for #ident #ty_generics #where_clause {
            type Payload<'__payload> = #payload_ty_in_gat;

            fn num_topics(&self) -> u32 {
                #num_topics
            }

            fn topic<'__event>(
                &'__event self,
                index: u32,
            ) -> ::core::option::Option<::sdk::TopicSrc<'__event>> {
                use ::sdk::IntoTopic as _;
                match index {
                    0 => ::core::option::Option::Some(
                        ::sdk::TopicSrc::Shared(Self::SIGNATURE)
                    ),
                    #(#topic_arms)*
                    _ => ::core::option::Option::None,
                }
            }

            fn decode_payload<'__payload>(
                bytes: &'__payload [u8],
            ) -> ::core::result::Result<#payload_ty_in_gat, ::sdk::ReadError> {
                ::sdk::__xdrs::cxs::decode(bytes)
            }
        }

        impl #impl_generics ::sdk::WriteXDR for #ident #ty_generics #where_clause {
            fn write_xdr<__F, __W: ::sdk::Writer<__F>>(
                &self,
                w: &mut __W,
            ) -> ::sdk::WriteResult<()> {
                #(#payload_writes)*
                ::core::result::Result::Ok(())
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn expand_emits_event_spec_fragment_and_anchors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TransferEvent<'a> {
                #[topic]
                from: &'a ContractAddress,
                #[topic]
                to: &'a ContractAddress,
                amount: u128,
                memo: &'a [u8],
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(tokens.contains("SpecFragmentView :: Event"), "{tokens}");
        assert!(tokens.contains("ContractEventDefView"), "{tokens}");
        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
        assert!(
            tokens
                .contains("__groundhog_spec_raw_event_fragment_TransferEvent"),
            "{tokens}"
        );
        assert!(
            tokens.contains("__GROUNDHOG_EVENT_SPEC_ANCHORS"),
            "{tokens}"
        );
        // Topic fields strip the reference and lower via the type-ref trait.
        assert!(
            tokens.contains("type_ref :: < ContractAddress >"),
            "{tokens}"
        );
        // Slice payload fields lower through the unsized trait impl.
        assert!(tokens.contains("type_ref :: < [u8] >"), "{tokens}");
    }

    #[test]
    fn expand_folds_nested_lifetimes_to_static_in_spec_types() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct WrappedEvent<'a> {
                data: Option<&'a [u8]>,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains("type_ref :: < Option < & 'static [u8] > >"),
            "{tokens}"
        );
        assert!(!tokens.contains("Option < & 'a [u8] > as"), "{tokens}");
    }

    #[test]
    fn expand_event_without_named_field_types_skips_anchors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct SimpleEvent {
                value: u128,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(tokens.contains("SpecFragmentView :: Event"), "{tokens}");
        // Primitive field types still get anchor entries (they are no-ops),
        // so the anchor array is present whenever the event has fields.
        assert!(
            tokens.contains("__GROUNDHOG_EVENT_SPEC_ANCHORS"),
            "{tokens}"
        );
    }
}
