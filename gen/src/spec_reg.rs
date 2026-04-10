use crate::Config;
use crate::contract::{
    ConfiguredContract, ContractSrc, Method, contract_type_typedef_tokens,
    effective_interface_name, method_arg_spec_tokens,
    method_return_spec_token, throws_spec_tokens,
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
        quote! { env!("CARGO_PKG_NAME") },
        iface_name.clone(),
        iface_vers.clone(),
        iface_throws,
    );
    let trait_throw_typedef_fragments =
        item.trait_.as_ref().map(|(_, trait_path, _)| {
            emit_raw_typedef_fragment_stream_tokens(quote! {
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROW_TYPE_DEFS
            })
        });
    let mut typedef_fragments = contract
        .throws_list()
        .into_iter()
        .map(|thrown| typedef_fragment_tokens_for_type_path(thrown.ty()))
        .collect::<Result<Vec<_>>>()?;
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                quote! { env!("CARGO_PKG_NAME") },
                iface_name.clone(),
                iface_vers.clone(),
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        typedef_fragments.extend(typedef_fragment_tokens_for_method(method)?);
    }

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
        #trait_throw_typedef_fragments
        #(#typedef_fragments)*
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
        quote! { env!("CARGO_PKG_NAME") },
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        quote! { env!("CARGO_PKG_NAME") },
        quote! { #iface_name },
        quote! { #iface_vers },
        item,
    )?;
    let mut typedef_fragments = contract
        .throws_list()
        .into_iter()
        .map(|thrown| typedef_fragment_tokens_for_type_path(thrown.ty()))
        .collect::<Result<Vec<_>>>()?;
    typedef_fragments.extend(typedef_fragment_tokens_for_method(item)?);

    Ok(quote! {
        #interface_fragment
        #method_fragment
        #(#typedef_fragments)*
    })
}

fn generate_interface_fragment_tokens(
    package_name: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    throws: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens(quote! {
        ::sdk::__contract_spec::SpecFragmentView::Interface(
            ::sdk::__contract_spec::SpecInterfaceFragmentView {
                package_name: #package_name,
                interface_name: #interface_name,
                interface_version: #interface_version,
                throws: #throws,
            }
        )
    })
}

fn generate_method_fragment_tokens(
    package_name: TokenStream,
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
                package_name: #package_name,
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
        quote! { all(target_family = "wasm", feature = "entrypoints") },
        fragment,
    )
}

pub fn emit_raw_fragment_tokens_with_cfg(
    cfg_guard: TokenStream,
    fragment: TokenStream,
) -> TokenStream {
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

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_SPEC_FRAGMENT: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
    }
}

pub fn emit_raw_typedef_fragment_stream_tokens(
    type_defs: TokenStream,
) -> TokenStream {
    quote! {
        #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
        const _: () = {
            const TYPE_DEFS: &[Option<::sdk::__contract_spec::ContractTypeDefView<'static>>] =
                #type_defs;
            const RAW_LEN: usize =
                ::sdk::__contract_spec::const_txs_typedef_fragment_stream_len(TYPE_DEFS);
            const RAW_FRAGMENT: [u8; RAW_LEN] =
                ::sdk::__contract_spec::const_txs_typedef_fragment_stream_bytes::<RAW_LEN>(TYPE_DEFS);

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_TYPE_DEF_FRAGMENT_STREAM: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
    }
}

fn typedef_fragment_tokens_for_method(
    method: &impl Method,
) -> Result<Vec<TokenStream>> {
    let mut fragments = Vec::new();

    for arg in &method.sig().inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            fragments.extend(typedef_fragment_tokens_for_type(
                pat_type.ty.as_ref(),
            )?);
        }
    }

    if let syn::ReturnType::Type(_, ty) = &method.sig().output {
        fragments.extend(typedef_fragment_tokens_for_type(ty.as_ref())?);
    }

    Ok(fragments)
}

fn typedef_fragment_tokens_for_type_path(
    path: &syn::Path,
) -> Result<TokenStream> {
    let ty = syn::Type::Path(syn::TypePath {
        qself: None,
        path: path.clone(),
    });
    let mut tokens = typedef_fragment_tokens_for_type(&ty)?;
    debug_assert!(tokens.len() <= 1);
    Ok(tokens.pop().unwrap_or_else(|| quote! {}))
}

fn typedef_fragment_tokens_for_type(
    ty: &syn::Type,
) -> Result<Vec<TokenStream>> {
    contract_type_typedef_tokens(ty).map(|type_defs| {
        type_defs
            .into_iter()
            .map(|type_def| {
                emit_raw_typedef_fragment_stream_tokens(
                    quote! { &[#type_def] },
                )
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> crate::Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        crate::Config::from_list(&meta_list).unwrap()
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
            &crate::Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("groundhogspecrawv0"));
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
        assert!(tokens.contains("SpecInterfaceFragmentView"));
        assert!(tokens.contains("SpecMethodFragmentView"));
        assert!(tokens.contains("const_txs_size"));
        assert!(tokens.contains("ConstWriter"));
        assert!(tokens.contains("ERC20"));
        assert!(tokens.contains("0.1.0"));
        assert!(tokens.contains("transfer"));
    }

    #[test]
    fn byte_slices_and_vecs_do_not_emit_u8_typedef_fragments() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                fn echo(data: &[u8]) -> Vec<u8> { data.into() }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &crate::Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(!tokens.contains("MaybeContractTypeDefSpec"));
    }

    #[test]
    fn builtin_byte_blob_types_use_trait_typedef_resolution() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn verify(
                    signature: &sdk::Ed25519Signature,
                    public_key: &sdk::Ed25519PublicKey,
                    entrypoint: &sdk::ContractEntrypointID,
                ) -> bool { true }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &crate::Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("maybe_type_def"));
        assert!(tokens.contains("Ed25519Signature"));
        assert!(tokens.contains("Ed25519PublicKey"));
        assert!(tokens.contains("ContractEntrypointID"));
    }

    #[test]
    fn block_height_uses_trait_typedef_resolution() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl Demo {
                pub fn deadline(height: sdk::BlockHeight) -> sdk::BlockHeight { height }
            }
        };

        let tokens = generate_impl_raw_spec_fragments(
            &item,
            &crate::Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("maybe_type_def"));
        assert!(tokens.contains("BlockHeight"));
    }
}
