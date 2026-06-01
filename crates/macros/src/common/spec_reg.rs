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
    let contract = ConfiguredContract::new(config.clone(), item);
    let self_ty = &item.self_ty;
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();

    let (iface_name, iface_vers, iface_throws) = if let Some((
        _,
        trait_path,
        _,
    )) = &item.trait_
    {
        (
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROWS },
        )
    } else {
        let iface_name = effective_interface_name(&contract);
        let iface_vers = contract.defaulted_interface_vers();
        let throws = throws_spec_tokens(&contract)?;
        (
            quote! { #iface_name },
            quote! { #iface_vers },
            quote! { &[#(#throws),*] },
        )
    };

    let interface_fragment = generate_interface_fragment_tokens(
        iface_name.clone(),
        iface_vers.clone(),
        iface_throws,
    );
    let mut anchor_roots = if let Some((_, trait_path, _)) = &item.trait_ {
        vec![quote! {
            <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROW_ANCHOR_ROOT
        }]
    } else {
        throws_anchor_tokens(&contract)?
    };
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                iface_name.clone(),
                iface_vers.clone(),
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        anchor_roots.extend(method_anchor_tokens(method)?);
    }
    let anchor_root =
        emit_root_anchor_tokens(&anchor_roots).unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
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
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        item,
    )?;
    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(method_anchor_tokens(item)?);
    let anchor_root =
        emit_root_anchor_tokens(&anchor_roots).unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #method_fragment
        #anchor_root
    })
}

fn generate_interface_fragment_tokens(
    interface_name: TokenStream,
    interface_version: TokenStream,
    throws: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens(quote! {
        ::sdk::__contract_spec::SpecFragmentView::Interface(
            ::sdk::__contract_spec::SpecInterfaceFragmentView {
                interface_name: #interface_name,
                interface_version: #interface_version,
                throws: #throws,
            }
        )
    })
}

fn generate_method_fragment_tokens(
    interface_name: TokenStream,
    interface_version: TokenStream,
    method: &impl Method,
) -> Result<TokenStream> {
    let method_name = method.sig().ident.to_string();
    let args = method_arg_spec_tokens(method)?;
    let return_type = method_return_spec_token(method)?;

    Ok(emit_raw_fragment_tokens(quote! {{
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
    }}))
}

pub fn emit_raw_fragment_tokens(fragment: TokenStream) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        quote! {
            all(
                any(target_family = "wasm", target_arch = "riscv32"),
                feature = "entrypoints"
            )
        },
        fragment,
    )
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
    anchor_roots: &[TokenStream],
) -> Option<TokenStream> {
    if anchor_roots.is_empty() {
        return None;
    }

    let count = anchor_roots.len();
    Some(quote! {
        #[cfg(all(any(target_family = "wasm", target_arch = "riscv32"), feature = "entrypoints"))]
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
}
