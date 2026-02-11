use darling::FromMeta;
use darling::ast::NestedMeta;
use groundhog_codegen::Config;
use groundhog_codegen::strip::*;
use groundhog_codegen::util::*;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::parse::Result;
use syn::parse_quote;

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
            // Inject the interface version as an associated const
            let mut stripped = item.strip_our_attributes();
            let iface_vers = groundhog_codegen::contract::interface_vers_default();
            stripped.items.push(parse_quote! {
                const __GROUNDHOG_IFACE_VERSION: &'static str = #iface_vers;
            });

            let mut out = stripped.into_token_stream();

            // Generate client interface
            out.extend(groundhog_codegen::client::generate(item, config)?);

            Ok(out)
        }
        syn::Item::Impl(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Generate exports
            out.extend(groundhog_codegen::exports::generate(item, config)?);

            if cfg!(feature = "mock") {
                // Generate test registration
                out.extend(groundhog_codegen::test_reg::generate(
                    item, config,
                )?);
            }

            if config.client_ident().is_some() {
                // Generate client interface
                out.extend(groundhog_codegen::client::generate(item, config)?);
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
            out.extend(groundhog_codegen::exports::generate_method(
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
