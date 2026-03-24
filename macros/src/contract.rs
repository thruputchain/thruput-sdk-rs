use codegen::Config;
use codegen::strip::*;
use codegen::util::*;
use darling::FromMeta;
use darling::ast::NestedMeta;
use groundhog_codegen as codegen;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident};
use syn::parse::Result;

pub fn apply(
    attr_args: TokenStream,
    contract: syn::Item,
) -> Result<TokenStream2> {
    // Parse the attributes to a `Config`
    let config: Config = {
        let meta_list = NestedMeta::parse_meta_list(attr_args.into())
            .map_err(darling::Error::from)?;
        Config::from_list(&meta_list)
    }?;

    apply_with_config(&contract, &config)
}

fn apply_with_config(
    item: &syn::Item,
    config: &Config,
) -> Result<TokenStream2> {
    match item {
        syn::Item::Trait(item) => {
            let stripped = item.strip_our_attributes();
            let iface_vers = codegen::contract::interface_vers_default();
            let iface_major =
                groundhog_contract_util::semver::major_version_from_string(
                    &iface_vers,
                );

            let trait_ident = &item.ident;
            let meta_ident = format_ident!("__{trait_ident}Meta");

            let mut out = stripped.into_token_stream();

            // Generate a companion struct that carries the version const,
            // keeping the trait itself dyn-compatible.
            out.extend(quote::quote! {
                #[doc(hidden)]
                pub struct #meta_ident;
                impl #meta_ident {
                    pub const IFACE_MAJOR_VERSION: &'static str = #iface_major;
                }
            });

            // Generate client interface
            out.extend(codegen::client::generate(item, config)?);

            Ok(out)
        }
        syn::Item::Impl(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Extract trait name for auto-deriving module names
            let trait_name = item
                .trait_
                .as_ref()
                .map(|(_, path, _)| {
                    path.segments.last().unwrap().ident.to_string()
                });

            // Generate exports (with auto-derived module name for trait impls)
            out.extend(codegen::exports::generate(
                item,
                config,
                trait_name.as_deref(),
            )?);

            if cfg!(feature = "mock") {
                // Generate test registration
                out.extend(codegen::test_reg::generate(
                    item,
                    config,
                    trait_name.as_deref(),
                )?);
            }

            if config.client_ident().is_some() {
                // It would be incorrect to generate a client interface here if the item is implementing a trait,
                // as the trait is already implementing a client interface.
                // It'd be too easy to double-generate the client struct.
                if item.trait_.is_some() {
                    out.extend(quote::quote! {
                        compile_error!("Adding an explicit client attribute to an impl of a trait is not allowed.  This would double-generate the client interface.  Add the client attribute to the trait instead.");
                    });
                    return Ok(out);
                }

                // Generate client interface
                out.extend(codegen::client::generate(item, config)?);
            } else if item.trait_.is_none() {
                out.extend(quote::quote! {
                    compile_error!("client = ClientName is required on an impl without a trait (no client interface would otherwise be generated)");
                });
                return Ok(out);
            }

            Ok(out)
        }
        syn::Item::Fn(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            let iface_name = config.interface_name();
            let iface_name = iface_name.as_deref();

            let contract: Option<&syn::ItemImpl> = None;

            // Generate exports
            out.extend(codegen::exports::generate_method(
                iface_name, contract, item, config,
            )?);

            Ok(out)
        }
        _ => Err(error(
            item,
            format!("Can't apply attribute to item {:?}", item),
        )),
    }
}
