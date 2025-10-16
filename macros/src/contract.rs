use darling::FromMeta;
use darling::ast::NestedMeta;
use gen::Config;
use gen::util::*;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::parse::Result;

pub fn generate_output(
    attr_args: TokenStream,
    item_tokens: TokenStream,
    contract: syn::Item,
) -> Result<TokenStream2> {
    // Parse the attributes to a `Config`
    let config: Config = {
        let meta_list = NestedMeta::parse_meta_list(attr_args.into())
            .map_err(darling::Error::from)?;
        Config::from_list(&meta_list)
    }?;

    // Generate the helper code
    let helpers = generate_helpers(&contract, &config)?;

    // Output the original item, followed by the helpers
    let mut output: TokenStream2 = item_tokens.into();
    output.extend(helpers);

    Ok(output)
}

fn generate_helpers(
    item: &syn::Item,
    config: &Config,
) -> Result<TokenStream2> {
    match item {
        syn::Item::Trait(item) => {
            // Generate client interface
            Ok(gen::client::generate(item, config)?)
        }
        syn::Item::Impl(item) => {
            let mut out = TokenStream2::new();

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
        syn::Item::Fn(item) => {
            // Get a macro-attribute-configured interface index, if any
            let index =
                config.interface_index().map(|i| i as usize).unwrap_or(0);

            let iface_name = config.interface_name();
            let iface_name = iface_name.as_deref();

            let contract: Option<&syn::ItemImpl> = None;

            // Generate exports
            Ok(gen::exports::generate_method(
                index, iface_name, contract, &item.sig, config,
            )?)
        }
        _ => Err(error(
            item,
            format!("Can't generate contract helpers for item {:?}", item),
        )),
    }
}
