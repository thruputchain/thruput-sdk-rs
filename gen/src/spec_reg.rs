use crate::Config;
use crate::contract::{
    ConfiguredContract, ContractSrc, Method, effective_interface_name,
    method_arg_spec_tokens, method_return_spec_token, throws_spec_tokens,
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
            generate_typedef_fragment_stream_tokens(quote! {
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROW_TYPE_DEFS
            })
        });
    let mut typedef_fragments = contract
        .throws_list()
        .into_iter()
        .map(|thrown| typedef_fragment_tokens_for_type_path(thrown.ty()))
        .collect::<Result<Vec<_>>>()?;
    let method_fragments = contract
        .methods_sorted()
        .into_iter()
        .map(|method| {
            generate_method_fragment_tokens(
                quote! { env!("CARGO_PKG_NAME") },
                iface_name.clone(),
                iface_vers.clone(),
                &method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in contract.methods_sorted() {
        typedef_fragments.extend(typedef_fragment_tokens_for_method(&method)?);
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
    quote! {
        #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
        const _: () = {
            const FRAGMENT: ::sdk::__contract_spec::SpecInterfaceFragmentView<'static> =
                ::sdk::__contract_spec::SpecInterfaceFragmentView {
                    package_name: #package_name,
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    throws: #throws,
                };
            const RAW_LEN: usize =
                ::sdk::internal::const_xdr_spec_interface_fragment_len(&FRAGMENT);
            const RAW_FRAGMENT: [u8; RAW_LEN] =
                ::sdk::internal::const_xdr_spec_interface_fragment_bytes::<RAW_LEN>(&FRAGMENT);

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_INTERFACE_SPEC: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
    }
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

    Ok(quote! {
        #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
        const _: () = {
            const METHOD_ARGS: &[::sdk::__contract_spec::ContractFieldView<'static>] =
                &[#(#args),*];
            const FRAGMENT: ::sdk::__contract_spec::SpecMethodFragmentView<'static> =
                ::sdk::__contract_spec::SpecMethodFragmentView {
                    package_name: #package_name,
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    method: ::sdk::__contract_spec::ContractSpecMethodView {
                        name: #method_name,
                        args: METHOD_ARGS,
                        return_type: #return_type,
                    },
                };
            const RAW_LEN: usize =
                ::sdk::internal::const_xdr_spec_method_fragment_len(&FRAGMENT);
            const RAW_FRAGMENT: [u8; RAW_LEN] =
                ::sdk::internal::const_xdr_spec_method_fragment_bytes::<RAW_LEN>(&FRAGMENT);

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_METHOD_SPEC: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
    })
}

fn generate_typedef_fragment_stream_tokens(
    type_defs: TokenStream,
) -> TokenStream {
    quote! {
        #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
        const _: () = {
            const TYPE_DEFS: &[Option<::sdk::__contract_spec::ContractTypeDefView<'static>>] =
                #type_defs;
            const RAW_LEN: usize =
                ::sdk::internal::const_xdr_spec_typedef_fragments_len(TYPE_DEFS);
            const RAW_FRAGMENT: [u8; RAW_LEN] =
                ::sdk::internal::const_xdr_spec_typedef_fragments_bytes::<RAW_LEN>(TYPE_DEFS);

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_TYPE_DEF_SPEC_STREAM: [u8; RAW_LEN] = RAW_FRAGMENT;
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
    match ty {
        syn::Type::Reference(reference) => {
            typedef_fragment_tokens_for_type(reference.elem.as_ref())
        }
        syn::Type::Paren(paren) => {
            typedef_fragment_tokens_for_type(paren.elem.as_ref())
        }
        syn::Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .map(typedef_fragment_tokens_for_type)
            .collect::<Result<Vec<_>>>()
            .map(|nested| nested.into_iter().flatten().collect()),
        syn::Type::Path(type_path) => {
            if path_last_segment_ident(&type_path.path) == "Option"
                || path_last_segment_ident(&type_path.path) == "Vec"
            {
                return first_generic_type_arg(type_path)
                    .map(typedef_fragment_tokens_for_type)
                    .unwrap_or_else(|| Ok(vec![]));
            }
            if path_last_segment_ident(&type_path.path) == "ContractAddress"
                || path_last_segment_ident(&type_path.path) == "SHA256Hash"
            {
                return Ok(vec![]);
            }
            Ok(vec![quote! {
                #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
                const _: () = {
                    const TYPE_DEF: Option<::sdk::__contract_spec::ContractTypeDefView<'static>> =
                        <#ty as ::sdk::__contract_spec::MaybeContractTypeDefSpec>::TYPE_DEF;
                    const RAW_LEN: usize = match TYPE_DEF {
                        Some(def) => ::sdk::internal::const_xdr_spec_typedef_fragment_len(&def),
                        None => 0,
                    };
                    const RAW_FRAGMENT: [u8; RAW_LEN] = match TYPE_DEF {
                        Some(def) => ::sdk::internal::const_xdr_spec_typedef_fragment_bytes::<RAW_LEN>(&def),
                        None => [0u8; RAW_LEN],
                    };

                    #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
                    pub static RAW_TYPE_DEF_SPEC: [u8; RAW_LEN] = RAW_FRAGMENT;
                };
            }])
        }
        _ => Ok(vec![]),
    }
}

fn path_last_segment_ident(path: &syn::Path) -> &syn::Ident {
    let seg = path.segments.last().unwrap();
    &seg.ident
}

fn first_generic_type_arg(type_path: &syn::TypePath) -> Option<&syn::Type> {
    let segment = type_path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
    else {
        return None;
    };
    arguments.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
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
        assert!(tokens.contains("const_xdr_spec_method_fragment_bytes"));
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
        assert!(tokens.contains("const_xdr_spec_interface_fragment_bytes"));
        assert!(tokens.contains("const_xdr_spec_method_fragment_bytes"));
        assert!(tokens.contains("ERC20"));
        assert!(tokens.contains("0.1.0"));
        assert!(tokens.contains("transfer"));
    }
}
