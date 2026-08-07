use super::common;
use common::client;
use common::config::Config;
use common::contract;
use common::contract::{ConfiguredContract, ContractSrc};
use common::exports;
use common::spec_reg;
use common::strip::*;
use common::test_reg;
use common::util::*;
use darling::FromMeta;
use darling::ast::NestedMeta;
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

    config.validate()?;

    apply_with_config(&contract, &config)
}

fn apply_with_config(
    item: &syn::Item,
    config: &Config,
) -> Result<TokenStream2> {
    match item {
        syn::Item::Trait(item) => {
            let contract = ConfiguredContract::new(config.clone(), item);
            contract::validate_reserved_client_method_names(
                contract.methods_sorted(),
            )?;
            contract::validate_method_arg_patterns(contract.methods_sorted())?;
            let throws_tokens = contract::throws_spec_tokens(&contract)?;
            let throw_anchor_tokens =
                contract::throws_anchor_tokens(&contract)?;
            let throw_anchor_stmts = throw_anchor_tokens.iter().map(|anchor| {
                quote::quote! {
                    acc = ::sdk::__contract_spec::mix_anchor_fn(acc, #anchor);
                }
            });

            let emits = config.emits_list();
            let event_def_tokens = emits.iter().map(|path| {
                quote::quote! {
                    <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF
                }
            });
            let event_anchor_stmts = emits.iter().map(|path| {
                quote::quote! {
                    acc = ::sdk::__contract_spec::mix_anchor_fn(
                        acc,
                        <#path as ::sdk::__contract_spec::ContractEventSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT,
                    );
                }
            });

            let mut stripped = item.strip_our_attributes();

            let iface_name = config
                .interface_name()
                .unwrap_or_else(|| item.ident.to_string());
            let iface_vers = config
                .version_string()
                .unwrap_or_else(contract::interface_vers_default);
            let iface_major =
                groundhog_contract_util::semver::major_version_from_string(
                    &iface_vers,
                );

            // Add the version consts on the trait (one `parse_quote!` per item — syn parses a
            // single `TraitItem`; two `const`s in one block panics with "unexpected token").
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_NAME: &'static str = #iface_name;
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_MAJOR_VERSION: &'static str = #iface_major;
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_FULL_VERSION: &'static str = #iface_vers;
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_THROWS: &'static [::sdk::__contract_spec::ContractExceptionTypeView<'static>] = &[#(#throws_tokens),*];
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                #[cfg_attr(target_arch = "riscv32", inline(never))]
                #[cfg_attr(
                    target_arch = "riscv32",
                    unsafe(link_section = "groundhogspecanchortextv0")
                )]
                fn __GROUNDHOG_IFACE_THROW_ANCHOR_ROOT() -> usize {
                    let mut acc = 0usize;
                    #(#throw_anchor_stmts)*
                    acc
                }
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_EVENTS: &'static [::sdk::__contract_spec::ContractEventDefView<'static>] = &[#(#event_def_tokens),*];
            });
            let emits_name_tokens = emits.iter().map(|path| {
                quote::quote! {
                    <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF.name
                }
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_EMITS: &'static [&'static str] = &[#(#emits_name_tokens),*];
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                #[cfg_attr(target_arch = "riscv32", inline(never))]
                #[cfg_attr(
                    target_arch = "riscv32",
                    unsafe(link_section = "groundhogspecanchortextv0")
                )]
                fn __GROUNDHOG_IFACE_EVENT_ANCHOR_ROOT() -> usize {
                    let mut acc = 0usize;
                    #(#event_anchor_stmts)*
                    acc
                }
            });

            // The trait is the authority for method signatures (names and
            // types). Emit them as a const that a trait impl pulls into its
            // artifact (mirroring __GROUNDHOG_IFACE_EVENTS), so an impl that
            // renames a parameter (e.g. `_factory`) cannot diverge the
            // canonical interface spec, and so impls of an external interface
            // crate still emit a complete spec.
            let method_views = contract
                .methods_sorted()
                .into_iter()
                .map(|m| contract::method_spec_view_tokens(&m))
                .collect::<Result<Vec<_>>>()?;
            let method_anchor_lists = contract
                .methods_sorted()
                .into_iter()
                .map(|m| contract::method_anchor_tokens(&m))
                .collect::<Result<Vec<_>>>()?;
            let method_anchor_stmts =
                method_anchor_lists.into_iter().flatten().map(|anchor| {
                    quote::quote! {
                        acc = ::sdk::__contract_spec::mix_anchor_fn(acc, #anchor);
                    }
                });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_METHODS: &'static [::sdk::__contract_spec::ContractSpecMethodView<'static>] = &[#(#method_views),*];
            });
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                #[cfg_attr(target_arch = "riscv32", inline(never))]
                #[cfg_attr(
                    target_arch = "riscv32",
                    unsafe(link_section = "groundhogspecanchortextv0")
                )]
                fn __GROUNDHOG_IFACE_METHOD_ANCHOR_ROOT() -> usize {
                    let mut acc = 0usize;
                    #(#method_anchor_stmts)*
                    acc
                }
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

            // A module-level anchor const (the trait isn't dyn-compatible, so
            // its associated `__GROUNDHOG_IFACE_*` consts aren't reachable as
            // `<dyn Trait>::…`). `export_interface_spec!` references this by
            // name to emit an InterfaceAnchor for a standalone interface carrier.
            let anchor_const_ident = quote::format_ident!(
                "__GROUNDHOG_IFACE_ANCHOR_{}",
                item.ident
            );
            out.extend(quote::quote! {
                #[doc(hidden)]
                #[allow(non_upper_case_globals)]
                pub const #anchor_const_ident:
                    ::sdk::__contract_spec::SpecInterfaceAnchorView<'static> =
                    ::sdk::__contract_spec::SpecInterfaceAnchorView {
                        interface_name: #iface_name,
                        interface_version: #iface_vers,
                    };
            });

            // Generate client interface
            out.extend(client::generate(item, config)?);
            out.extend(exports::generate_entrypoints(item, config)?);
            out.extend(spec_reg::generate_trait_raw_spec_fragments(
                item, config,
            )?);

            Ok(out)
        }
        syn::Item::Impl(item) => {
            let contract = ConfiguredContract::new(config.clone(), item);
            contract::validate_bare_impl_method_visibility(item)?;
            contract::validate_reserved_client_method_names(
                contract.methods_sorted(),
            )?;
            contract::validate_method_arg_patterns(contract.methods_sorted())?;
            let mut stripped = item.strip_our_attributes();

            // For trait impls, inject __mock into the impl block
            // BEFORE tokenizing.
            if cfg!(feature = "mock") && item.trait_.is_some() {
                stripped
                    .items
                    .push(test_reg::generate_trait_method(item, config)?);
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

            // Same for `emits`: events are part of the interface.
            if config.has_emits() && item.trait_.is_some() {
                let out = stripped.into_token_stream();
                return Ok(quote::quote! {
                    #out
                    compile_error!("`emits` on a trait impl is not allowed. Add `emits` to the trait definition instead, since the trait defines the interface.");
                });
            }

            let mut out = stripped.into_token_stream();

            // For bare impls, generate a separate impl block with __mock
            if cfg!(feature = "mock") && item.trait_.is_none() {
                out.extend(test_reg::generate_bare_method(item, config)?);
            }

            // Generate exports (with auto-derived module name for trait impls)
            out.extend(exports::generate(item, config)?);
            out.extend(spec_reg::generate_impl_raw_spec_fragments(
                item, config,
            )?);

            if config.client_ident().is_some() && item.trait_.is_some() {
                out.extend(quote::quote! {
                    compile_error!("Adding an explicit client attribute to an impl of a trait is not allowed.  This would double-generate the client interface.  Add the client attribute to the trait instead.");
                });
                return Ok(out);
            }

            if cfg!(feature = "mock") {
                out.extend(test_reg::generate_impl_registry_fragment(
                    item, config,
                )?);
            }

            if item.trait_.is_none() {
                // Bare impl: generate client (name from config or auto-derived)
                out.extend(client::generate(item, config)?);
            }

            Ok(out)
        }
        syn::Item::Fn(item) => {
            let contract = ConfiguredContract::new(config.clone(), item);
            contract::validate_reserved_client_method_names(
                contract.methods_sorted(),
            )?;
            contract::validate_method_arg_patterns(contract.methods_sorted())?;

            // Start with the item itself, stripped of proprietary attributes
            let mut out = item.strip_our_attributes().into_token_stream();

            out.extend(exports::generate_free_function_bundle(item, config)?);
            out.extend(spec_reg::generate_free_function_raw_spec_fragments(
                item, config,
            )?);

            if cfg!(feature = "mock") {
                out.extend(
                    test_reg::generate_free_function_registry_fragment(
                        item, config,
                    )?,
                );
            }

            Ok(out)
        }
        _ => Err(error(
            item,
            format!("Can't apply attribute to item {:?}", item),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use super::apply_with_config;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn trait_contract_emits_raw_spec_fragments() {
        let item: syn::Item = syn::parse_quote! {
            pub trait ERC20 {
                fn transfer(&mut self, receiver: &sdk::ContractAddress, amount: u128);
                fn total_supply(&self) -> u128;
            }
        };
        let tokens = apply_with_config(&item, &parse_config(""))
            .unwrap()
            .to_string();
        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
        assert!(tokens.contains("pub static RAW_SPEC_FRAGMENT"), "{tokens}");
        assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
        assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"), "{tokens}");
    }

    #[test]
    fn trait_contract_emits_authoritative_methods_const() {
        // The trait exposes its method signatures as a const so trait impls
        // pull the *trait's* names/types (not the impl's) into their spec.
        let item: syn::Item = syn::parse_quote! {
            pub trait Pair {
                fn initialize(&mut self, factory: &sdk::ContractAddress);
            }
        };
        let tokens = apply_with_config(&item, &parse_config(""))
            .unwrap()
            .to_string();
        assert!(tokens.contains("__GROUNDHOG_IFACE_METHODS"), "{tokens}");
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_METHOD_ANCHOR_ROOT"),
            "{tokens}"
        );
        assert!(tokens.contains("\"factory\""), "{tokens}");
    }

    #[test]
    fn trait_contract_emits_throw_anchor_root() {
        let item: syn::Item = syn::parse_quote! {
            pub trait ImportedErrorCallee {
                fn throw_imported_error();
            }
        };

        let config =
            parse_config(r#"throws(imported_error_type::ImportedError)"#);
        let tokens = apply_with_config(&item, &config).unwrap().to_string();

        assert!(
            tokens.contains("__GROUNDHOG_IFACE_THROW_ANCHOR_ROOT"),
            "{tokens}"
        );
        assert!(
            tokens.contains("ImportedError as :: sdk :: __contract_spec :: ContractTypeRefSpec"),
            "{tokens}"
        );
    }
}

#[cfg(test)]
mod emits_tests {
    use super::Config;
    use super::apply_with_config;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn trait_contract_emits_event_consts_and_anchor_root() {
        let item: syn::Item = syn::parse_quote! {
            pub trait Token {
                fn transfer(&mut self);
            }
        };

        let config =
            parse_config(r#"emits(TransferEvent, events::ApprovalEvent)"#);
        let tokens = apply_with_config(&item, &config).unwrap().to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_EVENTS"), "{tokens}");
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_EVENT_ANCHOR_ROOT"),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "TransferEvent as :: sdk :: __contract_spec :: ContractEventSpec > :: EVENT_DEF"
            ),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "events :: ApprovalEvent as :: sdk :: __contract_spec :: ContractEventSpec"
            ),
            "{tokens}"
        );
    }

    #[test]
    fn trait_contract_without_emits_declares_empty_events() {
        let item: syn::Item = syn::parse_quote! {
            pub trait Quiet {
                fn noop(&self);
            }
        };

        let tokens = apply_with_config(&item, &parse_config(""))
            .unwrap()
            .to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_EVENTS"), "{tokens}");
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_EVENT_ANCHOR_ROOT"),
            "{tokens}"
        );
    }

    #[test]
    fn trait_gets_emits_names_const() {
        let item: syn::Item = syn::parse_quote! {
            pub trait Token {
                fn transfer(&mut self);
            }
        };

        let config =
            parse_config(r#"emits(TransferEvent, events::ApprovalEvent)"#);
        let tokens = apply_with_config(&item, &config).unwrap().to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_EMITS"), "{tokens}");
        assert!(
            tokens.contains("ContractEventSpec > :: EVENT_DEF . name"),
            "{tokens}"
        );
    }

    #[test]
    fn emits_on_trait_impl_is_rejected() {
        let item: syn::Item = syn::parse_quote! {
            impl Token for Demo {
                fn transfer(&mut self) {}
            }
        };

        let config = parse_config(r#"emits(TransferEvent)"#);
        let tokens = apply_with_config(&item, &config).unwrap().to_string();

        assert!(
            tokens.contains("`emits` on a trait impl is not allowed"),
            "{tokens}"
        );
    }
}
