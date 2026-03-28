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

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Event)] only supports structs",
        ));
    };

    let (topics, payload) = partition_fields(&data.fields)?;

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
