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
    if let Some((trait_path, _)) = &item.trait_ {
        let declared_by = quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_DECLARED_BY };
        let interface_fragment = generate_interface_fragment_tokens(
            &entrypoints_cfg(),
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
            declared_by.clone(),
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROWS },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_EMITS },
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
            declared_by,
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
    // No trait declares this interface: the contract crate itself does.
    let declared_by = crate::common::spec_id::crate_id_tokens(
        &crate::common::contract::sdk_crate_path(),
        Some(&iface_vers),
    );

    let emits_names = config_emits_name_tokens(config);
    let interface_fragment = generate_interface_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        declared_by.clone(),
        quote! { &[#(#throws),*] },
        quote! { &[#(#emits_names),*] },
    );
    let event_fragments =
        generate_config_event_fragments_tokens(&entrypoints_cfg(), config);
    // A bare impl is its own interface; anchor it so it survives shaking.
    let interface_anchor = generate_interface_anchor_tokens(
        entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        declared_by.clone(),
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
                declared_by.clone(),
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
    // No trait declares this interface: the contract crate itself does.
    let declared_by = crate::common::spec_id::crate_id_tokens(
        &crate::common::contract::sdk_crate_path(),
        Some(&iface_vers),
    );

    let emits_names = config_emits_name_tokens(config);
    let interface_fragment = generate_interface_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        declared_by.clone(),
        quote! { &[#(#throws),*] },
        quote! { &[#(#emits_names),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        &entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        declared_by.clone(),
        item,
    )?;
    let event_fragments =
        generate_config_event_fragments_tokens(&entrypoints_cfg(), config);
    let interface_anchor = generate_interface_anchor_tokens(
        entrypoints_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        declared_by.clone(),
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

/// Emit the `groundhogspecrawv3` fragments for a `#[contract]` trait
/// (interface) definition: one Interface fragment, event fragments, plus
/// throws/event/signature anchor roots. Method fragments come from the
/// anchoring site instead (see the comment in the body). Name/version match
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

    // A trait emits its interface/event fragments as *candidates*,
    // target-gated (not `entrypoints`): present whenever the interface crate
    // compiles for a contract target, with no feature required, and retained
    // via `#[used]`. Post-processing drops them unless something anchors the
    // interface — a `#[contract] impl` does this, and a standalone interface
    // carrier does it explicitly via `export_interface_spec!`.
    //
    // Method fragments are deliberately NOT among them. Two unrelated traits
    // that collide on the interface key (default name = trait ident, default
    // version = crate version) merge into one bucket, so candidate methods
    // from a trait nothing implements would surface in the artifact's spec as
    // phantom methods — advertising an entrypoint that does not exist. Only
    // an anchoring site emits methods: a `#[contract] impl` re-emits the
    // trait's authoritative `__GROUNDHOG_IFACE_METHODS` (see
    // `generate_trait_method_fragments_tokens`), and `export_interface_spec!`
    // emits them from the module-level twin of that const.
    // The trait's id, read from the module-level anchor const the
    // `#[contract]` trait emits beside it, so this candidate and every
    // anchor of the trait agree on it by construction.
    let anchor_const =
        quote::format_ident!("__GROUNDHOG_IFACE_ANCHOR_{}", item.ident);
    let emits_names = config_emits_name_tokens(config);
    let interface_fragment = generate_interface_fragment_tokens(
        &target_cfg(),
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { #anchor_const.declared_by },
        quote! { &[#(#throws),*] },
        quote! { &[#(#emits_names),*] },
    );
    let event_fragments =
        generate_config_event_fragments_tokens(&target_cfg(), config);

    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(config_event_anchor_tokens(config));

    // Signature types stay link-GC roots even though the method fragments
    // themselves are not emitted here: a carrier anchoring this interface
    // needs their `SpecTypeDef` fragments present in the artifact. Unreached
    // type defs are shaken in post-processing, so keeping them costs nothing
    // in the published spec.
    for method in &methods {
        anchor_roots.extend(method_anchor_tokens(method)?);
    }
    let anchor_root = emit_root_anchor_tokens(&target_cfg(), &anchor_roots)
        .unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
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
        #[cfg(all(target_arch = "riscv32", feature = "entrypoints"))]
        const _: () = {
            const METHODS: &[::sdk::__contract_spec::ContractSpecMethodView<'static>] =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_METHODS;
            const NAME: &str = <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME;
            const VERS: &str =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION;
            const DECLARED_BY:
                ::sdk::__contract_spec::ContractSpecIdView<'static> =
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_DECLARED_BY;
            const RAW_LEN: usize = {
                let mut total = 0usize;
                let mut i = 0;
                while i < METHODS.len() {
                    total += ::sdk::__contract_spec::SpecFragmentView::Method(
                        ::sdk::__contract_spec::SpecMethodFragmentView {
                            interface_name: NAME,
                            interface_version: VERS,
                            declared_by: DECLARED_BY,
                            method: METHODS[i],
                        },
                    )
                    .const_txs_size();
                    i += 1;
                }
                total
            };

            #[cfg_attr(
                target_arch = "riscv32",
                unsafe(link_section = "groundhogspecrawv3")
            )]
            // `#[used]` keeps the section live through ELF linking so the
            // CLI can extract it.
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
                            declared_by: DECLARED_BY,
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
        #[cfg(all(target_arch = "riscv32", feature = "entrypoints"))]
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
                target_arch = "riscv32",
                unsafe(link_section = "groundhogspecrawv3")
            )]
            // `#[used]` keeps the section live through ELF linking so the
            // CLI can extract it.
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
    quote! { target_arch = "riscv32" }
}

/// The cfg gate for an *anchor*: emitted only at the site that wants the
/// interface in its own artifact's spec (a contract built with `entrypoints`,
/// or an interface carrier built with `spec`).
fn entrypoints_cfg() -> TokenStream {
    quote! {
        all(target_arch = "riscv32", feature = "entrypoints")
    }
}

/// Emit an `InterfaceAnchor` plus the trait's `Method` fragments,
/// target-gated. Used by the `export_interface_spec!` macro in an interface
/// carrier (the trait's `main.rs`) so building the interface crate on its own
/// yields its spec.
///
/// The methods must be emitted here, not at the trait definition: a trait
/// nothing implements contributes no methods to any artifact's spec (see
/// `generate_trait_raw_spec_fragments`), and a carrier is the one place that
/// wants an unimplemented interface published in full.
///
/// Resolves the module-level `__GROUNDHOG_IFACE_ANCHOR_<Trait>` and
/// `__GROUNDHOG_IFACE_METHODS_<Trait>` consts the `#[contract]` trait emits
/// (the trait itself isn't dyn-compatible, so its associated consts aren't
/// reachable without an implementor).
pub fn generate_interface_anchor_for_trait(path: &syn::Path) -> TokenStream {
    let trait_const = |prefix: &str| {
        let mut const_path = path.clone();
        if let Some(last) = const_path.segments.last_mut() {
            last.ident = quote::format_ident!("{prefix}{}", last.ident);
            last.arguments = syn::PathArguments::None;
        }
        const_path
    };
    let anchor_path = trait_const("__GROUNDHOG_IFACE_ANCHOR_");
    let methods_path = trait_const("__GROUNDHOG_IFACE_METHODS_");

    let anchor = emit_raw_fragment_tokens_with_cfg(
        target_cfg(),
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::InterfaceAnchor(#anchor_path)
        },
    );
    let methods =
        generate_carrier_method_fragments_tokens(&anchor_path, &methods_path);

    quote! {
        #anchor
        #methods
    }
}

/// Concatenated `Method` fragments for an interface carrier, read from the
/// trait's module-level method const. Mirrors
/// `generate_trait_method_fragments_tokens` (the impl-side twin); the
/// name/version come from the same anchor const the carrier anchors with, so
/// the two can never disagree.
fn generate_carrier_method_fragments_tokens(
    anchor_path: &syn::Path,
    methods_path: &syn::Path,
) -> TokenStream {
    let cfg = target_cfg();
    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    quote! {
        #[cfg(#cfg)]
        const _: () = {
            const METHODS: &[::sdk::__contract_spec::ContractSpecMethodView<'static>] =
                #methods_path;
            const NAME: &str = #anchor_path.interface_name;
            const VERS: &str = #anchor_path.interface_version;
            const DECLARED_BY:
                ::sdk::__contract_spec::ContractSpecIdView<'static> =
                #anchor_path.declared_by;
            const RAW_LEN: usize = {
                let mut total = 0usize;
                let mut i = 0;
                while i < METHODS.len() {
                    total += ::sdk::__contract_spec::SpecFragmentView::Method(
                        ::sdk::__contract_spec::SpecMethodFragmentView {
                            interface_name: NAME,
                            interface_version: VERS,
                            declared_by: DECLARED_BY,
                            method: METHODS[i],
                        },
                    )
                    .const_txs_size();
                    i += 1;
                }
                total
            };

            #[cfg_attr(
                target_arch = "riscv32",
                unsafe(link_section = "groundhogspecrawv3")
            )]
            // `#[used]` keeps the section live through ELF linking so the
            // CLI can extract it.
            #[cfg_attr(target_arch = "riscv32", used)]
            pub static RAW_SPEC_CARRIER_METHOD_FRAGMENTS: [u8; RAW_LEN] = {
                let mut w = ::sdk::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
                let mut i = 0;
                while i < METHODS.len() {
                    ::sdk::__contract_spec::SpecFragmentView::Method(
                        ::sdk::__contract_spec::SpecMethodFragmentView {
                            interface_name: NAME,
                            interface_version: VERS,
                            declared_by: DECLARED_BY,
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

/// Emit an `InterfaceAnchor` fragment for `(name, version)` under `cfg`. The
/// post-processor keeps an interface's candidate spec only if it is anchored.
fn generate_interface_anchor_tokens(
    cfg: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    declared_by: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        cfg,
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::InterfaceAnchor(
                ::sdk::__contract_spec::SpecInterfaceAnchorView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    declared_by: #declared_by,
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

/// Event-name reference tokens for the `emits` field of an interface
/// fragment, from the `emits(...)` config list, in declaration order.
fn config_emits_name_tokens(config: &Config) -> Vec<TokenStream> {
    config
        .emits_list()
        .into_iter()
        .map(|path| {
            quote! {
                <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF.name
            }
        })
        .collect()
}

fn generate_interface_fragment_tokens(
    cfg: &TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    declared_by: TokenStream,
    throws: TokenStream,
    emits: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        cfg.clone(),
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::Interface(
                ::sdk::__contract_spec::SpecInterfaceFragmentView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    declared_by: #declared_by,
                    throws: #throws,
                    emits: #emits,
                }
            )
        },
    )
}

fn generate_method_fragment_tokens(
    cfg: &TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    declared_by: TokenStream,
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
                    declared_by: #declared_by,
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
                target_arch = "riscv32",
                unsafe(link_section = "groundhogspecrawv3")
            )]
            // `#[used]` keeps the section live through ELF linking so the CLI
            // can extract it.
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
            // This table is a link-time GC root only; contract code never
            // reads it. On RV32 the linker follows its relocations to pull in
            // the transitive raw-spec graph, then discards the table itself.
            #[cfg_attr(
                target_arch = "riscv32",
                unsafe(link_section = "groundhogspecanchorsv0")
            )]
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

        assert!(tokens.contains("groundhogspecrawv3"));
        assert!(tokens.contains("target_arch = \"riscv32\""));
        assert!(tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"));
        assert!(tokens.contains("SpecInterfaceFragmentView"));
        assert!(tokens.contains("SpecMethodFragmentView"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_FULL_VERSION"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_THROWS"));
        assert!(tokens.contains("const_txs_size"));
        assert!(tokens.contains("ConstWriter"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_EMITS"), "{tokens}");
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

        assert!(tokens.contains("groundhogspecrawv3"));
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
        assert!(tokens.contains("groundhogspecanchorsv0"), "{tokens}");
        assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
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
    fn trait_fragments_carry_the_interface_but_not_its_methods() {
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

        assert!(tokens.contains("groundhogspecrawv3"), "{tokens}");
        assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
        assert!(
            tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"),
            "{tokens}"
        );
        assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");

        // Methods come from the anchoring site, never from the definition:
        // an unimplemented trait must contribute no methods to the spec of
        // an artifact that merely links it. Two traits sharing an interface
        // key would otherwise merge into one bucket and publish each other's
        // methods as phantoms.
        assert!(!tokens.contains("SpecMethodFragmentView"), "{tokens}");
        assert!(!tokens.contains("\"total_supply\""), "{tokens}");
        assert!(!tokens.contains("\"transfer\""), "{tokens}");
        assert!(!tokens.contains("\"receiver\""), "{tokens}");

        // The signature types stay link-GC roots so a carrier anchoring this
        // interface still finds their type defs in the artifact.
        assert!(tokens.contains("ROOT_SPEC_ANCHORS"), "{tokens}");
    }

    /// The carrier is the one site that publishes an interface nobody
    /// implements, so it must emit the method fragments the trait definition
    /// no longer does — keyed off the same anchor const it anchors with.
    #[test]
    fn carrier_anchor_emits_the_traits_methods() {
        let path: syn::Path = syn::parse_quote!(groundhog_erc20_if::ERC20);
        let tokens = generate_interface_anchor_for_trait(&path).to_string();

        assert!(
            tokens.contains("__GROUNDHOG_IFACE_ANCHOR_ERC20"),
            "{tokens}"
        );
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_METHODS_ERC20"),
            "{tokens}"
        );
        assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
        assert!(
            tokens.contains(
                "used)] pub static RAW_SPEC_CARRIER_METHOD_FRAGMENTS"
            ),
            "{tokens}"
        );
        // Name and version are read back off the anchor const, so a carrier
        // cannot key its methods to a different interface than it anchors.
        assert!(
            tokens.contains("__GROUNDHOG_IFACE_ANCHOR_ERC20 . interface_name"),
            "{tokens}"
        );
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
        assert!(
            tokens.contains("EVENT_DEF . name"),
            "interface fragment must carry emits names: {tokens}"
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
        assert!(tokens.contains("groundhogspecrawv3"), "{tokens}");
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
