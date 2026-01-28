use darling::FromMeta;
use darling::ast::NestedMeta;
use gen::Config;
use gen::strip::*;
use gen::util::*;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
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
        // For a trait, generate:
        // - client interface
        syn::Item::Trait(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Generate client interface
            out.extend(gen::client::generate(item, config)?);

            Ok(out)
        }

        // For a trait impl or inherent impl, generate:
        // - entrypoints
        // - test registration
        // - client interface
        syn::Item::Impl(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Generate exports
            out.extend(gen::exports::generate(item, config)?);

            if cfg!(feature = "mock") {
                // Generate test registration
                out.extend(gen::test_reg::generate(item, config)?);
            }

            if config.client_ident().is_some() {
                // Generate client interface
                out.extend(gen::client::generate(item, config)?);
            }

            Ok(out)
        }

        // For a module, generate:
        // - entrypoints
        // - test registration
        // - client interface?
        syn::Item::Mod(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Generate exports
            out.extend(gen::exports::generate(item, config)?);

            if cfg!(feature = "mock") {
                // Generate test registration
                out.extend(gen::test_reg::generate(item, config)?);
            }

            if config.client_ident().is_some() {
                // Generate client interface
                out.extend(gen::client::generate(item, config)?);
            }

            Ok(out)
        }

        // For a function item, generate:
        // - entrypoint
        syn::Item::Fn(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Get a macro-attribute-configured interface index, if any
            let index =
                config.interface_index().map(|i| i as usize).unwrap_or(0);

            let iface_name = config.interface_name();
            let iface_name = iface_name.as_deref();

            let contract: Option<&syn::ItemImpl> = None;

            // Generate entrypoint
            out.extend(gen::exports::generate_method(
                index, iface_name, contract, item, config,
            )?);

            Ok(out)
        }

        _ => Err(error(
            item,
            format!("Can't apply attribute to item {:?}", item),
        )),
    }
}
