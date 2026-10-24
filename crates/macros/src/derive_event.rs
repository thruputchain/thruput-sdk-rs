use crate::common::contract::{
    contract_type_anchor_token, contract_type_view_tokens,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{DeriveInput, Fields, GenericParam, Result};

#[derive(Debug, Default, darling::FromMeta)]
struct EventSpecArgs {
    #[darling(default)]
    export: bool,
    wire_crate: Option<darling::util::SpannedValue<String>>,
}

fn event_spec_args(input: &DeriveInput) -> Result<EventSpecArgs> {
    use darling::FromMeta;
    let mut result = EventSpecArgs::default();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("event_spec"))
    {
        let args = EventSpecArgs::from_meta(&attr.meta)
            .map_err(|err| syn::Error::new_spanned(attr, err.to_string()))?;
        result.export |= args.export;
        if let Some(name) = args.wire_crate {
            if result.wire_crate.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "wire_crate may be specified only once",
                ));
            }
            crate::common::util::validate_wire_crate_name(name.span(), &name)?;
            result.wire_crate = Some(name);
        }
    }
    Ok(result)
}

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
        ::thruput_sdk::__contract_spec::ContractFieldView {
            name: #name,
            ty: #ty_tokens,
        }
    })
}

/// Generates the `ContractEventSpec` impl (the event's spec definition as a
/// const view) plus an anchor fn for its field types. The definition is only
/// written to the raw spec section where the event is declared: from an
/// interface's `emits(...)` list, or — when `#[event_spec(export)]` is set —
/// unconditionally from the defining crate (forced inclusion).
fn emit_event_spec(
    input: &DeriveInput,
    type_name: &str,
    sig_bytes: &[u8; 32],
    topics: &[&syn::Field],
    payload: &[&syn::Field],
    export: bool,
) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    // The same id every spec-named item carries. An interface's `emits(...)`
    // entry reads this def's `name`, so the reference follows the event here
    // rather than repeating its ident. The `signature` below is untouched: it
    // is the on-wire topic-0 discriminator, hashed from the wire crate, major
    // version and ident, and moving it would break every deployed indexer.
    let id = crate::common::spec_id::spec_id_tokens(
        &crate::common::contract::sdk_crate_path(),
        type_name,
        // `#[derive(Event)]` declares no version of its own, and its signature
        // is hashed from the crate's, so the id follows the same.
        None,
    );
    let def_ident = format_ident!("__thruput_spec_event_def_{ident}");
    let anchor_fn_ident =
        format_ident!("__thruput_spec_event_anchor_fn_{ident}");

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
    let anchor_stmts = anchor_tokens.iter().map(|anchor| {
        quote! {
            acc = ::thruput_sdk::__contract_spec::mix_anchor_fn(acc, #anchor);
        }
    });

    let exported_fragment = if export {
        emit_exported_event_fragment(ident, &def_ident, &anchor_fn_ident)
    } else {
        quote! {}
    };

    let sig_array = sig_bytes.iter().copied();

    Ok(quote! {
        #[doc(hidden)]
        pub const #def_ident: ::thruput_sdk::__contract_spec::ContractEventDefView<'static> =
            ::thruput_sdk::__contract_spec::ContractEventDefView {
                name: #id,
                signature: [#(#sig_array),*],
                topics: &[#(#topic_views),*],
                fields: &[#(#payload_views),*],
            };

        #[doc(hidden)]
        #[inline(never)]
        #[cfg_attr(
            target_arch = "riscv32",
            unsafe(link_section = "thruputspecanchortextv0")
        )]
        pub fn #anchor_fn_ident() -> usize {
            #[cfg(target_arch = "riscv32")]
            {
                let mut acc = 0usize;
                #(#anchor_stmts)*
                acc
            }
            #[cfg(not(target_arch = "riscv32"))]
            {
                0usize
            }
        }

        impl #impl_generics ::thruput_sdk::__contract_spec::ContractEventSpec for #ident #ty_generics #where_clause {
            const EVENT_DEF: ::thruput_sdk::__contract_spec::ContractEventDefView<'static> =
                #def_ident;

            const __THRUPUT_SPEC_ANCHOR_ROOT: fn() -> usize = #anchor_fn_ident;

            fn __thruput_spec_anchor() -> usize {
                #anchor_fn_ident()
            }
        }

        #exported_fragment
    })
}

/// Emits a raw `ExportedSpecEvent` fragment for `#[event_spec(export)]`,
/// forcing the event into the contract spec of any binary that links the
/// defining crate, mirroring `derive_contract_type_spec`'s exported typedefs.
fn emit_exported_event_fragment(
    ident: &syn::Ident,
    def_ident: &syn::Ident,
    anchor_fn_ident: &syn::Ident,
) -> TokenStream {
    let fragment_ident =
        format_ident!("__thruput_spec_event_fragment_{ident}");
    let raw_len_ident = format_ident!("__thruput_spec_event_raw_len_{ident}");
    let raw_fragment_ident =
        format_ident!("__thruput_spec_raw_event_fragment_{ident}");

    // `link_section` requires a string literal. Keep this in sync with
    // `thruput_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    // On RV32 this is an ALLOC data section, retained by `#[used]`, and the
    // CLI's rv32 lowerer excludes it from VM memory before artifact packing.
    quote! {
        #[doc(hidden)]
        pub const #fragment_ident: ::thruput_sdk::__contract_spec::SpecFragmentView<'static> =
            ::thruput_sdk::__contract_spec::SpecFragmentView::ExportedEvent(#def_ident);

        #[doc(hidden)]
        const #raw_len_ident: usize = #fragment_ident.const_txs_size();

        #[cfg(target_arch = "riscv32")]
        #[doc(hidden)]
        #[unsafe(link_section = "groundhogspecrawv4")]
        // `#[used]` keeps the section live through ELF linking so the CLI
        // can extract it.
        #[used]
        static #raw_fragment_ident: [u8; #raw_len_ident] = {
            let mut w =
                ::thruput_sdk::__xdrs::txs::ConstWriter::<#raw_len_ident>::new();
            #fragment_ident.const_write_txs(&mut w);
            w.into_bytes()
        };

        // Retains the raw typedef fragments of the event's field types, so
        // `Named` references in the exported def resolve during spec assembly.
        #[cfg(target_arch = "riscv32")]
        const _: () = {
            // Link-time retention metadata only. The RV32 linker follows the
            // relocation to pull in the raw-spec graph, then discards the
            // table before producing the final ELF.
            #[cfg_attr(
                target_arch = "riscv32",
                unsafe(link_section = "thruputspecanchorsv0")
            )]
            #[used]
            static __THRUPUT_EVENT_SPEC_ANCHORS: [fn() -> usize; 1] =
                [#anchor_fn_ident];
        };
    }
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    // The event def is a non-generic module-level const, so a type-scoped
    // `T` would surface as an unresolvable type inside generated code.
    // Lifetimes are supported (folded to `'static` in the spec const).
    crate::common::util::validate_no_generics(
        &input.generics,
        true,
        "#[derive(Event)] types",
    )?;
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Event)] only supports structs",
        ));
    };

    let args = event_spec_args(input)?;
    let (topics, payload) = partition_fields(&data.fields)?;

    let max_topics = thruput_xdr::event::kMaxTopics as usize;
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
    let crate_name = args
        .wire_crate
        .as_ref()
        .map(|name| name.to_string())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_NAME")
                .unwrap_or_else(|_| "_unknown_".to_string())
        });
    let sig_bytes = thruput_contract_util::event_signature(
        &version,
        &crate_name,
        &type_name,
    );

    let sig_array = sig_bytes.iter().copied();

    // --- Spec definition (ContractEventSpec impl + optional exported fragment) ---
    let event_spec = emit_event_spec(
        input,
        &type_name,
        &sig_bytes,
        &topics,
        &payload,
        args.export,
    )?;

    // --- Event impl ---
    let num_topics = (topics.len() + 1) as u32; // +1 for SIGNATURE at index 0

    let topic_arms = topics.iter().enumerate().map(|(i, field)| {
        let index = (i + 1) as u32; // 0 is SIGNATURE
        let field_ident = field.ident.as_ref().unwrap();
        // Spanned to the field's type so an unsupported topic reports there
        // rather than at the `derive(Event)` line, which names neither the
        // field nor the trait.
        //
        // Fully qualified, so a `#[topic]` field with no impl fails the
        // `IntoTopic` bound at the field and picks up the trait's
        // `on_unimplemented`. Method-call syntax resolved through auto-deref
        // instead, which reports a bare E0599 with no note; the impls for
        // `ContractAddress` and `&T` in event.rs are what make the qualified
        // form accept every real topic type.
        let call = quote::quote_spanned! { field.ty.span() =>
            <_ as ::thruput_sdk::event::IntoTopic>::into_topic(&self.#field_ident)
        };
        quote! {
            #index => ::core::option::Option::Some(#call),
        }
    });

    // --- WriteXDR impl ---
    let payload_writes = payload.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            ::thruput_sdk::WriteXDR::write_xdr(&self.#field_ident, w)?;
        }
    });
    let payload_writes_unchecked = payload.iter().map(|field| {
        let field_ident = field.ident.as_ref().unwrap();
        quote! {
            unsafe {
                ::thruput_sdk::WriteXDR::write_xdr_unchecked(&self.#field_ident, w);
            }
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
        #event_spec

        impl #impl_generics #ident #ty_generics #where_clause {
            pub const SIGNATURE: &'static ::thruput_sdk::Topic = &[#(#sig_array),*];
        }

        #[derive(::thruput_sdk::ReadXDR)]
        #[xdrs(crate = "::thruput_sdk::__xdrs")]
        #[allow(dead_code)]
        #vis struct #payload_ident #payload_generics {
            #(#payload_fields,)*
            #[allow(dead_code)]
            __phantom: ::core::marker::PhantomData<( #(&#lifetime_params ()),* )>,
        }

        impl #impl_generics ::thruput_sdk::Event for #ident #ty_generics #where_clause {
            type Payload<'__payload> = #payload_ty_in_gat;

            fn num_topics() -> u32 {
                #num_topics
            }

            fn signature() -> ::core::option::Option<&'static ::thruput_sdk::Topic> {
                ::core::option::Option::Some(Self::SIGNATURE)
            }

            fn topic<'__event>(
                &'__event self,
                index: u32,
            ) -> ::core::option::Option<::thruput_sdk::TopicSrc<'__event>> {
                use ::thruput_sdk::IntoTopic as _;
                match index {
                    0 => ::core::option::Option::Some(
                        ::thruput_sdk::TopicSrc::Shared(Self::SIGNATURE)
                    ),
                    #(#topic_arms)*
                    _ => ::core::option::Option::None,
                }
            }

            fn decode_payload<'__payload>(
                bytes: &'__payload [u8],
            ) -> ::core::result::Result<#payload_ty_in_gat, ::thruput_sdk::ReadError> {
                ::thruput_sdk::__xdrs::cxs::decode(bytes)
            }
        }

        impl #impl_generics ::thruput_sdk::WriteXDR for #ident #ty_generics #where_clause {
            fn write_xdr<__F, __W: ::thruput_sdk::Writer<__F>>(
                &self,
                w: &mut __W,
            ) -> ::thruput_sdk::WriteResult<()> {
                #(#payload_writes)*
                ::core::result::Result::Ok(())
            }

            #[allow(unused_variables)]
            unsafe fn write_xdr_unchecked<
                __F,
                __W: ::thruput_sdk::__xdrs::WriterUnchecked<__F>,
            >(
                &self,
                w: &mut __W,
            ) {
                #(#payload_writes_unchecked)*
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn ambiguous_or_invalid_wire_crates_are_rejected() {
        let duplicate: syn::DeriveInput = syn::parse_quote! {
            #[event_spec(wire_crate = "first-package")]
            #[event_spec(wire_crate = "second-package")]
            struct Event { amount: u32 }
        };
        assert!(
            super::expand(&duplicate)
                .unwrap_err()
                .to_string()
                .contains("only once")
        );
        for name in ["", "other::package", "white space"] {
            let invalid: syn::DeriveInput = syn::parse_quote! {
                #[event_spec(wire_crate = #name)]
                struct Event { amount: u32 }
            };
            assert!(
                super::expand(&invalid)
                    .unwrap_err()
                    .to_string()
                    .contains("wire_crate must be")
            );
        }
    }

    use super::expand;

    #[test]
    fn expand_rejects_generic_type_parameters() {
        // Lifetimes are supported (folded to 'static in the spec const);
        // type/const parameters would splice an unresolvable `T` into the
        // module-level def const — reject with a clear message instead.
        let input: syn::DeriveInput = syn::parse_quote! {
            struct BadEvent<T> {
                value: T,
            }
        };
        let err = expand(&input).map(|t| t.to_string()).unwrap_err();
        assert!(err.to_string().contains("generic"), "{err}");
    }

    #[test]
    fn expand_emits_event_def_const_and_spec_impl() {
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

        assert!(
            tokens.contains("__thruput_spec_event_def_TransferEvent"),
            "{tokens}"
        );
        assert!(tokens.contains("ContractEventDefView"), "{tokens}");
        assert!(
            tokens.contains("ContractEventSpec for TransferEvent"),
            "{tokens}"
        );
        assert!(tokens.contains("thruputspecanchortextv0"), "{tokens}");
        assert!(
            tokens.contains("__thruput_spec_event_anchor_fn_TransferEvent"),
            "{tokens}"
        );
        // Topic fields strip the reference and lower via the type-ref trait.
        assert!(
            tokens.contains("type_ref :: < ContractAddress >"),
            "{tokens}"
        );
        // Slice payload fields lower through the unsized trait impl.
        assert!(tokens.contains("type_ref :: < [u8] >"), "{tokens}");
        // Without `#[event_spec(export)]` no raw fragment is emitted.
        assert!(!tokens.contains("groundhogspecrawv4"), "{tokens}");
    }

    #[test]
    fn expand_with_export_emits_exported_raw_fragment() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[event_spec(export)]
            struct AuditEvent {
                tag: u64,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains("SpecFragmentView :: ExportedEvent"),
            "{tokens}"
        );
        assert!(tokens.contains("groundhogspecrawv4"), "{tokens}");
        assert!(
            tokens.contains("__thruput_spec_raw_event_fragment_AuditEvent"),
            "{tokens}"
        );
        assert!(tokens.contains("__THRUPUT_EVENT_SPEC_ANCHORS"), "{tokens}");
        assert!(tokens.contains("thruputspecanchorsv0"), "{tokens}");
        assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
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
}
