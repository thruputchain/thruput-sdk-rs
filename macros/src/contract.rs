use codegen::Config;
use codegen::strip::*;
use codegen::util::*;
use darling::FromMeta;
use darling::ast::NestedMeta;
use groundhog_codegen as codegen;
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
        syn::Item::Trait(original_item) => {
            let mut item = original_item.strip_our_attributes();

            // Inject the major version as an associated const
            let iface_vers =
                groundhog_codegen::contract::interface_vers_default();
            let iface_major =
                groundhog_contract_util::semver::major_version_from_string(
                    &iface_vers,
                );
            item.items.push(parse_quote! {
                const __GROUNDHOG_IFACE_MAJOR_VERSION: &'static str = #iface_major;
            });

            // Inject a Throws supertrait
            item.supertraits.push(codegen::throws(config));

            // First output the augmented trait
            let mut out = item.into_token_stream();

            // Next, generate the client interface
            out.extend(codegen::client::generate(original_item, config)?);

            Ok(out)
        }
        syn::Item::Impl(item) => {
            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            // Generate exports
            out.extend(codegen::exports::generate(item, config)?);

            if cfg!(feature = "mock") {
                // Generate test registration
                out.extend(codegen::test_reg::generate(item, config)?);
            }

            // Check that the config is legal
            if item.trait_.is_some() {
                // As we're implementing a trait specified elsewhere, we
                // should *not* specify an exception type here:
                if config.throws().is_some() {
                    return Err(error(item, "Please specify the 'throws' type \
                        on the trait, not the impl"));
                }
            }

            if config.client_ident().is_some() {
                // Generate client interface
                out.extend(codegen::client::generate(item, config)?);
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
            format!("Can't apply contract macro to this kind of item: {:?}", item),
        )),
    }
}
