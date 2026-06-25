use super::config::Config;
use super::contract::{
    ConfiguredContract, ContractSrc, Method, effective_interface_name,
    method_anchor_tokens, method_arg_spec_tokens, method_return_spec_token,
    throws_anchor_tokens, throws_spec_tokens,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

pub fn generate_impl_raw_spec_fragments(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let self_ty = &item.self_ty;

    // Trait impl: the trait is authoritative for the interface, its methods,
    // throws, and events. Pull every part from the trait's consts — including
    // `__GROUNDHOG_IFACE_METHODS` — rather than the impl's own signatures, so
    // an impl that renames a parameter (e.g. `_factory`) cannot diverge the
    // canonical spec. The trait consts are plain associated data, available
    // cross-crate without the interface crate being built with `entrypoints`,
    // so impls of an external interface still emit a complete spec.
    if let Some((_, trait_path, _)) = &item.trait_ {
        let interface_fragment = generate_interface_fragment_tokens(
            &entrypoints_cfg(),
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROWS },
        );
        let method_fragments =
            generate_trait_method_fragments_tokens(self_ty, trait_path);
        let event_fragments =
            generate_trait_event_fragments_tokens(self_ty, trait_path);
        // This impl anchors the interface, so the trait's candidate spec
        // (pulled in from the interface crate) survives post-processing.
        let interface_anchor = generate_interface_anchor_tokens(
            entrypoints_cfg(),
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
        );
        let anchor_roots = vec![
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROW_ANCHOR_ROOT },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_EVENT_ANCHOR_ROOT },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_METHOD_ANCHOR_ROOT },
        ];
        let anchor_root =
            emit_root_anchor_tokens(&entrypoints_cfg(), &anchor_roots)
                .unwrap_or_else(|| quote! {});
        return Ok(quote! {
            #interface_fragment
            #method_fragments
            #event_fragments
            #interface_anchor
            #anchor_root
        });
    }

    // Bare impl: emit from the impl's own signatures.
    let contract = ConfiguredContract::new(config.clone(), item);
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();
    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    let interface_fragment = generate_interface_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let event_fragments =
        generate_config_event_fragments_tokens(&entrypoints_cfg(), config);
    // A bare impl is its own interface; anchor it so it survives shaking.
    let interface_anchor = generate_interface_anchor_tokens(
        entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
    );
    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(config_event_anchor_tokens(config));
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                &entrypoints_cfg(),
                quote! { #iface_name },
                quote! { #iface_vers },
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        anchor_roots.extend(method_anchor_tokens(method)?);
    }
    let anchor_root =
        emit_root_anchor_tokens(&entrypoints_cfg(), &anchor_roots)
            .unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
        #event_fragments
        #interface_anchor
        #anchor_root
    })
}

pub fn generate_free_function_raw_spec_fragments(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    let interface_fragment = generate_interface_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        item,
    )?;
    let event_fragments =
        generate_config_event_fragments_tokens(&entrypoints_cfg(), config);
    let interface_anchor = generate_interface_anchor_tokens(
        entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
    );
    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(method_anchor_tokens(item)?);
    anchor_roots.extend(config_event_anchor_tokens(config));
    let anchor_root =
        emit_root_anchor_tokens(&entrypoints_cfg(), &anchor_roots)
            .unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #method_fragment
        #event_fragments
        #interface_anchor
        #anchor_root
    })
}

/// Emit the `groundhogspecrawv0` fragments for a `#[contract]` trait
/// (interface) definition: one Interface fragment, one Method fragment per
/// trait method, event fragments, plus throws/event/signature anchor roots.
/// Mirrors the bare-impl path of `generate_impl_raw_spec_fragments`, but
/// methods come from the trait and there is no `self_ty`. Name/version match
/// the trait branch's `__GROUNDHOG_IFACE_*` consts, so a trait-derived spec is
/// byte-identical in format to the spec a concrete impl would carry.
pub fn generate_trait_raw_spec_fragments(
    item: &syn::ItemTrait,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();

    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    // A trait emits its interface/method/event fragments as *candidates*,
    // target-gated (not `entrypoints`): present whenever the interface crate
    // compiles for a contract target, with no feature required, and retained
    // via `#[used]`. Post-processing drops them unless something anchors the
    // interface — a `#[contract] impl` does this, and a standalone interface
    // carrier does it explicitly via `export_interface_spec!`.
    let interface_fragment = generate_interface_fragment_tokens(
        &target_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let event_fragments =
        generate_config_event_fragments_tokens(&target_cfg(), config);

    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(config_event_anchor_tokens(config));

    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                &target_cfg(),
                quote! { #iface_name },
                quote! { #iface_vers },
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        anchor_roots.extend(method_anchor_tokens(method)?);
    }
    let anchor_root = emit_root_anchor_tokens(&target_cfg(), &anchor_roots)
        .unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
        #event_fragments
        #anchor_root
    })
}

/// Emits `SpecMethod` fragments for a trait impl from the trait's
/// authoritative `__GROUNDHOG_IFACE_METHODS` const (possibly in another
/// crate), wrapped with the trait's name/version. Mirrors
/// `generate_trait_event_fragments_tokens`: the fragments are concatenated
/// into a single raw byte array in const eval. Because the method specs come
/// from the trait — not the impl's signatures — an impl that renames a
/// parameter cannot diverge the spec, and the trait-side and impl-side
/// fragments are identical (so they dedupe when both land in one artifact).
fn generate_trait_method_fragments_tokens(
    self_ty: &syn::Type,
    trait_path: &syn::Path,
) -> TokenStream {
    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    quote! {
        #[cfg(all(
            any(target_family = "wasm", target_arch = "riscv32"),
            feature = "entrypoints"
        ))]
        const _: () = {
            const METHODS: &[::sdk::__contract_spec::ContractSpecMethodView<'static>] =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_METHODS;
            const NAME: &str = <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME;
            const VERS: &str =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION;
            const RAW_LEN: usize = {
                let mut total = 0usize;
                let mut i = 0;
                while i < METHODS.len() {
                    total += ::sdk::__contract_spec::SpecFragmentView::Method(
                        ::sdk::__contract_spec::SpecMethodFragmentView {
                            interface_name: NAME,
                            interface_version: VERS,
                            method: METHODS[i],
                        },
                    )
                    .const_txs_size();
                    i += 1;
                }
                total
            };

            #[cfg_attr(
                any(target_family = "wasm", target_arch = "riscv32"),
                unsafe(link_section = "groundhogspecrawv0")
            )]
            // RV32 only: `#[used]` keeps the section live through ELF linking
            // for CLI extraction. On wasm the custom section is retained
            // without it, and `#[used]` there would force the fragment bytes
            // into linear memory (tripping the spec-tool leak guard).
            #[cfg_attr(target_arch = "riscv32", used)]
            pub static RAW_SPEC_METHOD_FRAGMENTS: [u8; RAW_LEN] = {
                let mut w =
                    ::sdk::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
                let mut i = 0;
                while i < METHODS.len() {
                    ::sdk::__contract_spec::SpecFragmentView::Method(
                        ::sdk::__contract_spec::SpecMethodFragmentView {
                            interface_name: NAME,
                            interface_version: VERS,
                            method: METHODS[i],
                        },
                    )
                    .const_write_txs(&mut w);
                    i += 1;
                }
                w.into_bytes()
            };
        };
    }
}

/// Emits `SpecEvent` fragments for the events declared on a contract trait
/// via `emits(...)`. The declared defs live in the trait's
/// `__GROUNDHOG_IFACE_EVENTS` const (possibly in another crate), so the
/// fragments are concatenated into a single raw byte array in const eval.
fn generate_trait_event_fragments_tokens(
    self_ty: &syn::Type,
    trait_path: &syn::Path,
) -> TokenStream {
    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    quote! {
        #[cfg(all(
            any(target_family = "wasm", target_arch = "riscv32"),
            feature = "entrypoints"
        ))]
        const _: () = {
            const EVENTS: &[::sdk::__contract_spec::ContractEventDefView<'static>] =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_EVENTS;
            const RAW_LEN: usize = {
                let mut total = 0usize;
                let mut i = 0;
                while i < EVENTS.len() {
                    total += ::sdk::__contract_spec::SpecFragmentView::Event(
                        EVENTS[i],
                    )
                    .const_txs_size();
                    i += 1;
                }
                total
            };

            #[cfg_attr(
                any(target_family = "wasm", target_arch = "riscv32"),
                unsafe(link_section = "groundhogspecrawv0")
            )]
            // RV32 only: `#[used]` keeps the section live through ELF linking
            // for CLI extraction. On wasm the custom section is retained
            // without it, and `#[used]` there would force the fragment bytes
            // into linear memory (tripping the spec-tool leak guard).
            #[cfg_attr(target_arch = "riscv32", used)]
            pub static RAW_SPEC_EVENT_FRAGMENTS: [u8; RAW_LEN] = {
                let mut w =
                    ::sdk::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
                let mut i = 0;
                while i < EVENTS.len() {
                    ::sdk::__contract_spec::SpecFragmentView::Event(EVENTS[i])
                        .const_write_txs(&mut w);
                    i += 1;
                }
                w.into_bytes()
            };
        };
    }
}

/// The cfg gate for an interface's *candidate* spec fragments (interface,
/// methods, events). These are emitted whenever the defining crate compiles
/// for a contract target — independent of `entrypoints` — and are shaken out
/// in post-processing unless an impl anchors the interface.
fn target_cfg() -> TokenStream {
    quote! { any(target_family = "wasm", target_arch = "riscv32") }
}

/// The cfg gate for an *anchor*: emitted only at the site that wants the
/// interface in its own artifact's spec (a contract built with `entrypoints`,
/// or an interface carrier built with `spec`).
fn entrypoints_cfg() -> TokenStream {
    quote! {
        all(
            any(target_family = "wasm", target_arch = "riscv32"),
            feature = "entrypoints"
        )
    }
}

/// Emit an `InterfaceAnchor` for a trait, target-gated. Used by the
/// `export_interface_spec!` macro in an interface carrier (the trait's
/// `main.rs`) so building the interface crate on its own yields its spec.
///
/// Resolves the module-level `__GROUNDHOG_IFACE_ANCHOR_<Trait>` const the
/// `#[contract]` trait emits (the trait itself isn't dyn-compatible, so its
/// associated consts aren't reachable without an implementor).
pub fn generate_interface_anchor_for_trait(path: &syn::Path) -> TokenStream {
    let mut const_path = path.clone();
    if let Some(last) = const_path.segments.last_mut() {
        last.ident =
            quote::format_ident!("__GROUNDHOG_IFACE_ANCHOR_{}", last.ident);
        last.arguments = syn::PathArguments::None;
    }
    emit_raw_fragment_tokens_with_cfg(
        target_cfg(),
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::InterfaceAnchor(#const_path)
        },
    )
}

/// Emit an `InterfaceAnchor` fragment for `(name, version)` under `cfg`. The
/// post-processor keeps an interface's candidate spec only if it is anchored.
fn generate_interface_anchor_tokens(
    cfg: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        cfg,
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::InterfaceAnchor(
                ::sdk::__contract_spec::SpecInterfaceAnchorView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                }
            )
        },
    )
}

/// Emits one `SpecEvent` fragment per event declared via `emits(...)` on a
/// bare impl or free-function contract.
fn generate_config_event_fragments_tokens(
    cfg: &TokenStream,
    config: &Config,
) -> TokenStream {
    let fragments = config.emits_list().into_iter().map(|path| {
        emit_raw_fragment_tokens_with_cfg(cfg.clone(), quote! {
            ::sdk::__contract_spec::SpecFragmentView::Event(
                <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF
            )
        })
    });
    quote! { #(#fragments)* }
}

/// Anchor roots retaining typedef fragments for the field types of events
/// declared via `emits(...)` on a bare impl or free-function contract.
fn config_event_anchor_tokens(config: &Config) -> Vec<TokenStream> {
    config
        .emits_list()
        .into_iter()
        .map(|path| {
            quote! {
                <#path as ::sdk::__contract_spec::ContractEventSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT
            }
        })
        .collect()
}

fn generate_interface_fragment_tokens(
    cfg: &TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    throws: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        cfg.clone(),
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::Interface(
                ::sdk::__contract_spec::SpecInterfaceFragmentView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    throws: #throws,
                }
            )
        },
    )
}

fn generate_method_fragment_tokens(
    cfg: &TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    method: &impl Method,
) -> Result<TokenStream> {
    let method_name = method.sig().ident.to_string();
    let args = method_arg_spec_tokens(method)?;
    let return_type = method_return_spec_token(method)?;

    Ok(emit_raw_fragment_tokens_with_cfg(
        cfg.clone(),
        quote! {{
            const METHOD_ARGS: &[::sdk::__contract_spec::ContractFieldView<'static>] =
                &[#(#args),*];
            ::sdk::__contract_spec::SpecFragmentView::Method(
                ::sdk::__contract_spec::SpecMethodFragmentView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    method: ::sdk::__contract_spec::ContractSpecMethodView {
                        name: #method_name,
                        args: METHOD_ARGS,
                        return_type: #return_type,
                    },
                }
            )
        }},
    ))
}

pub fn emit_raw_fragment_tokens_with_cfg(
    cfg_guard: TokenStream,
    fragment: TokenStream,
) -> TokenStream {
    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    // On RV32 this is an ALLOC data section, retained by `#[used]`, and the
    // groundhog-rpc lowerer excludes it from VM memory before artifact packing.
    quote! {
        #[cfg(#cfg_guard)]
        const _: () = {
            const FRAGMENT: ::sdk::__contract_spec::SpecFragmentView<'static> =
                #fragment;
            const RAW_LEN: usize = FRAGMENT.const_txs_size();
            const RAW_FRAGMENT: [u8; RAW_LEN] = {
                let mut w = ::sdk::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
                FRAGMENT.const_write_txs(&mut w);
                w.into_bytes()
            };

            #[cfg_attr(
                any(target_family = "wasm", target_arch = "riscv32"),
                unsafe(link_section = "groundhogspecrawv0")
            )]
            // RV32 only: `#[used]` keeps the section live through ELF linking for
            // CLI extraction. On wasm the custom section is retained without it,
            // and `#[used]` there would force the fragment bytes into linear
            // memory (tripping the spec-tool leak guard).
            #[cfg_attr(target_arch = "riscv32", used)]
            pub static RAW_SPEC_FRAGMENT: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
    }
}

fn emit_root_anchor_tokens(
    cfg: &TokenStream,
    anchor_roots: &[TokenStream],
) -> Option<TokenStream> {
    if anchor_roots.is_empty() {
        return None;
    }

    let count = anchor_roots.len();
    Some(quote! {
        #[cfg(#cfg)]
        const _: () = {
            #[used]
            static ROOT_SPEC_ANCHORS: [fn() -> usize; #count] = [
                #(#anchor_roots),*
            ];
        };
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn trait_impl_fragments_include_interface_and_method_records() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl named_interface_callee::NamedInterfaceCallee for Demo {
                fn add_one(x: u32) -> u32 { x + 1 }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("groundhogspecrawv0"));
        assert!(tokens.contains("target_arch = \"riscv32\""));
        assert!(tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"));
        assert!(tokens.contains("SpecInterfaceFragmentView"));
        assert!(tokens.contains("SpecMethodFragmentView"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_FULL_VERSION"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_THROWS"));
        assert!(tokens.contains("const_txs_size"));
        assert!(tokens.contains("ConstWriter"));
    }

    #[test]
    fn free_function_fragments_include_method_record() {
        let item: syn::ItemFn = syn::parse_quote! {
            fn transfer(to: &sdk::ContractAddress, amount: u32) -> u32 { amount }
        };
        let config = parse_config(r#"interface = "ERC20", version = "0.1.0""#);

        let tokens = generate_free_function_raw_spec_fragments(&item, &config)
            .unwrap()
            .to_string();

        assert!(tokens.contains("groundhogspecrawv0"));
        assert!(tokens.contains("target_arch = \"riscv32\""));
        assert!(tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"));
        assert!(tokens.contains("SpecInterfaceFragmentView"));
        assert!(tokens.contains("SpecMethodFragmentView"));
        assert!(tokens.contains("const_txs_size"));
        assert!(tokens.contains("ConstWriter"));
        assert!(tokens.contains("ERC20"));
        assert!(tokens.contains("0.1.0"));
        assert!(tokens.contains("transfer"));
    }

    #[test]
    fn byte_composite_shapes_lower_without_trait_specialization() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn maybe_bytes(data: Option<&[u8]>) -> Option<Vec<u8>> {
                    data.map(Into::into)
                }

                pub fn tuple_bytes(data: (Vec<u8>, bool)) -> (Vec<u8>, bool) {
                    data
                }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(
            tokens.contains("type_ref :: < Option < & [u8] > >"),
            "{tokens}"
        );
        assert!(
            tokens.contains("type_ref :: < Option < Vec < u8 > > >"),
            "{tokens}"
        );
        assert!(
            tokens.contains("type_ref :: < (Vec < u8 > , bool) >"),
            "{tokens}"
        );
    }

    #[test]
    fn array_shapes_lower_via_trait_resolution() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn digest(data: [u8; 32], flags: [bool; 4]) -> [u8; 32] {
                    data
                }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("type_ref :: < [u8 ; 32] >"), "{tokens}");
        assert!(tokens.contains("type_ref :: < [bool ; 4] >"), "{tokens}");
    }

    #[test]
    fn alias_byte_collections_emit_trait_calls() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn echo_blob(blob: Blob) -> Blob {
                    blob
                }

                pub fn echo_hash(hash: Hash) -> Hash {
                    hash
                }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("type_ref :: < Blob >"), "{tokens}");
        assert!(tokens.contains("type_ref :: < Hash >"), "{tokens}");
    }

    #[test]
    fn impl_fragments_include_signature_anchor_roots() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn echo(payload: Option<Vec<Payload>>) -> Payload {
                    payload.unwrap().pop().unwrap()
                }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("__GROUNDHOG_SPEC_ANCHOR_ROOT"), "{tokens}");
        assert!(tokens.contains("Option < Vec < Payload > >"), "{tokens}");
    }

    #[test]
    fn trait_impl_fragments_reference_throw_anchor_root() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl imported_error_callee::ImportedErrorCallee for Demo {
                fn throw_imported_error() {}
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(
            tokens.contains("__GROUNDHOG_IFACE_THROW_ANCHOR_ROOT"),
            "{tokens}"
        );
    }

    #[test]
    fn trait_fragments_include_interface_and_method_records() {
        let item: syn::ItemTrait = syn::parse_quote! {
            pub trait ERC20 {
                fn transfer(&mut self, receiver: &sdk::ContractAddress, amount: u128);
                fn total_supply(&self) -> u128;
            }
        };
        let tokens = generate_trait_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
        assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
        assert!(
            tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"),
            "{tokens}"
        );
        assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
        assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
        assert!(tokens.contains("\"total_supply\""), "{tokens}");
        assert!(tokens.contains("\"transfer\""), "{tokens}");
        assert!(tokens.contains("\"receiver\""), "{tokens}");
    }

    #[test]
    fn trait_fragments_carry_config_throws_and_emits() {
        let item: syn::ItemTrait = syn::parse_quote! {
            pub trait Token { fn transfer(&mut self); }
        };
        let config = parse_config(
            r#"throws(err::TokenError), emits(events::TransferEvent)"#,
        );
        let tokens = generate_trait_raw_spec_fragments(&item, &config)
            .unwrap()
            .to_string();

        assert!(tokens.contains("ContractExceptionTypeView"), "{tokens}");
        assert!(
            tokens.contains("events :: TransferEvent as :: sdk :: __contract_spec :: ContractEventSpec > :: EVENT_DEF"),
            "{tokens}"
        );
    }

    #[test]
    fn trait_impl_methods_come_from_trait_const_not_impl_signatures() {
        // The impl renames the trait's `factory` param to `_factory`. Method
        // specs must be pulled from the trait's authoritative
        // `__GROUNDHOG_IFACE_METHODS` const, so the impl's `_factory` must not
        // leak into the emitted spec (otherwise a same-crate trait+impl would
        // emit conflicting method definitions).
        let item: syn::ItemImpl = syn::parse_quote! {
            impl pair_if::Pair for Demo {
                fn initialize(&mut self, _factory: u32) {}
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_METHODS"), "{tokens}");
        assert!(tokens.contains("RAW_SPEC_METHOD_FRAGMENTS"), "{tokens}");
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_METHOD_ANCHOR_ROOT"),
            "{tokens}"
        );
        // Methods are trait-sourced, so the impl's param name never appears.
        assert!(!tokens.contains("_factory"), "{tokens}");
    }
}

#[cfg(test)]
mod emits_tests {
    use super::*;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn trait_impl_emits_declared_event_fragments_and_anchor_root() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl token_if::Token for Demo {
                fn transfer(&mut self) {}
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_EVENTS"), "{tokens}");
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_EVENT_ANCHOR_ROOT"),
            "{tokens}"
        );
        assert!(tokens.contains("RAW_SPEC_EVENT_FRAGMENTS"), "{tokens}");
        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
    }

    #[test]
    fn bare_impl_emits_config_declared_event_fragments() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn touch(&mut self) {}
            }
        };

        let config = parse_config(r#"emits(events::TouchEvent)"#);
        let tokens = generate_impl_raw_spec_fragments(&item, &config)
            .unwrap()
            .to_string();

        assert!(tokens.contains("SpecFragmentView :: Event"), "{tokens}");
        assert!(
            tokens.contains(
                "events :: TouchEvent as :: sdk :: __contract_spec :: ContractEventSpec > :: EVENT_DEF"
            ),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "ContractEventSpec > :: __GROUNDHOG_SPEC_ANCHOR_ROOT"
            ),
            "{tokens}"
        );
    }

    #[test]
    fn free_function_emits_config_declared_event_fragments() {
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let config = parse_config(r#"emits(PingEvent)"#);
        let tokens = generate_free_function_raw_spec_fragments(&item, &config)
            .unwrap()
            .to_string();

        assert!(tokens.contains("SpecFragmentView :: Event"), "{tokens}");
        assert!(
            tokens.contains(
                "PingEvent as :: sdk :: __contract_spec :: ContractEventSpec"
            ),
            "{tokens}"
        );
    }
}
