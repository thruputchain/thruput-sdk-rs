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

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
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

    Ok(quote! {
        #interface_fragment
        #method_fragment
    })
}

fn generate_interface_fragment_tokens(
    package_name: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    throws: TokenStream,
) -> TokenStream {
    quote! {
        #[cfg(target_family = "wasm")]
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
        #[cfg(target_family = "wasm")]
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
