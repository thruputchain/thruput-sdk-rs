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
        syn::Item::Trait(item) => {
            let mut stripped = item.strip_our_attributes();

            let iface_vers = codegen::contract::interface_vers_default();
            let iface_major =
                groundhog_contract_util::semver::major_version_from_string(
                    &iface_vers,
                );

            // Add the version consts on the trait (one `parse_quote!` per item — syn parses a
            // single `TraitItem`; two `const`s in one block panics with "unexpected token").
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_MAJOR_VERSION: &'static str = #iface_major;
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_FULL_VERSION: &'static str = #iface_vers;
            });

            if cfg!(feature = "mock") {
                stripped.items.push(parse_quote! {
                    #[doc(hidden)]
                    fn __mock(contract: &mut ::sdk::MockContract) {
                        panic!("__mock not available \u{2014} use #[contract] on the impl");
                    }
                });
            }

            let mut out = stripped.into_token_stream();

            // Generate client interface
            out.extend(codegen::client::generate(item, config)?);

            Ok(out)
        }
        syn::Item::Impl(item) => {
            let mut stripped = item.strip_our_attributes();

            // Extract trait name for auto-deriving module names
            let trait_name = item.trait_.as_ref().map(|(_, path, _)| {
                path.segments.last().unwrap().ident.to_string()
            });

            // For trait impls, inject __mock into the impl block
            // BEFORE tokenizing.
            if cfg!(feature = "mock")
                && item.trait_.is_some()
                && let Some(method) = codegen::test_reg::generate_trait_method(
                    item,
                    config,
                    trait_name.as_deref(),
                )?
            {
                stripped.items.push(method);
            }

            // Reject `throws` on trait impls early, before generating
            // any code, so the user sees a single clear error.
            if config.throws().is_some() && item.trait_.is_some() {
                let out = stripped.into_token_stream();
                return Ok(quote::quote! {
                    #out
                    compile_error!("`throws` on a trait impl is not allowed. Add `throws` to the trait definition instead, since the trait defines the interface.");
                });
            }

            let mut out = stripped.into_token_stream();

            // For bare impls, generate a separate impl block with __mock
            if cfg!(feature = "mock") && item.trait_.is_none() {
                out.extend(codegen::test_reg::generate_bare_method(
                    item, config,
                )?);
            }

            // Generate exports (with auto-derived module name for trait impls)
            out.extend(codegen::exports::generate(
                item,
                config,
                trait_name.as_deref(),
            )?);

            if config.client_ident().is_some() && item.trait_.is_some() {
                out.extend(quote::quote! {
                    compile_error!("Adding an explicit client attribute to an impl of a trait is not allowed.  This would double-generate the client interface.  Add the client attribute to the trait instead.");
                });
                return Ok(out);
            }

            if item.trait_.is_none() {
                // Bare impl: generate client (name from config or auto-derived)
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
            format!("Can't apply attribute to item {:?}", item),
        )),
    }
}
