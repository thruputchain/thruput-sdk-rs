use crate::Config;
use crate::contract::{
    ConfiguredContract, ContractSrc, effective_interface_name,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

pub fn generate_impl_raw_spec_fragment(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let self_ty = &item.self_ty;

    let (iface_name, iface_vers) = if let Some((_, trait_path, _)) =
        &item.trait_
    {
        (
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
        )
    } else {
        let contract = ConfiguredContract::new(config.clone(), item);
        let iface_name = effective_interface_name(&contract);
        let iface_vers = contract.defaulted_interface_vers();
        (quote! { #iface_name }, quote! { #iface_vers })
    };

    Ok(generate_raw_fragment_tokens(
        quote! { env!("CARGO_PKG_NAME") },
        iface_name,
        iface_vers,
    ))
}

pub fn generate_free_function_raw_spec_fragment(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();

    Ok(generate_raw_fragment_tokens(
        quote! { env!("CARGO_PKG_NAME") },
        quote! { #iface_name },
        quote! { #iface_vers },
    ))
}

fn generate_raw_fragment_tokens(
    package_name: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
) -> TokenStream {
    quote! {
        #[cfg(all(target_family = "wasm", feature = "entrypoints"))]
        const _: () = {
            const PACKAGE_NAME: &str = #package_name;
            const INTERFACE_NAME: &str = #interface_name;
            const INTERFACE_VERSION: &str = #interface_version;
            const RAW_FRAGMENT: [u8; {
                9 + PACKAGE_NAME.len()
                    + INTERFACE_NAME.len()
                    + INTERFACE_VERSION.len()
            }] = ::sdk::internal::const_spec_raw_interface_fragment_bytes::<
                { PACKAGE_NAME.len() },
                { INTERFACE_NAME.len() },
                { INTERFACE_VERSION.len() },
                {
                    9 + PACKAGE_NAME.len()
                        + INTERFACE_NAME.len()
                        + INTERFACE_VERSION.len()
                },
            >(
                PACKAGE_NAME,
                INTERFACE_NAME,
                INTERFACE_VERSION,
            );

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_INTERFACE_SPEC: [u8; {
                9 + PACKAGE_NAME.len()
                    + INTERFACE_NAME.len()
                    + INTERFACE_VERSION.len()
            }] = RAW_FRAGMENT;
        };
    }
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
    fn trait_impl_fragments_use_trait_metadata_consts() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl named_interface_callee::NamedInterfaceCallee for Demo {
                fn add_one(x: u32) -> u32 { x + 1 }
            }
        };

        let tokens = generate_impl_raw_spec_fragment(
            &item,
            &crate::Config::from_list(&[]).unwrap(),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_FULL_VERSION"));
        assert!(tokens.contains("groundhogspecrawv0"));
    }

    #[test]
    fn free_function_fragments_use_effective_interface_name() {
        let item: syn::ItemFn = syn::parse_quote! {
            fn transfer() {}
        };
        let config = parse_config(r#"interface = "ERC20", version = "0.1.0""#);

        let tokens = generate_free_function_raw_spec_fragment(&item, &config)
            .unwrap()
            .to_string();

        assert!(tokens.contains("groundhogspecrawv0"));
        assert!(tokens.contains("ERC20"));
        assert!(tokens.contains("0.1.0"));
    }
}
