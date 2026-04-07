//! Generate exports

use super::Config;
use super::contract::{
    ConfiguredContract, ContractSrc, Method, effective_interface_name,
};
use super::entrypoint::*;
use super::util::*;
use groundhog_contract_util as contract_util;
use groundhog_xdr::contract::ContractEntrypointID;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use std::collections::HashMap;
use syn::LitByteStr;
use syn::parse::Result;

/// Generate exports
pub fn generate(
    contract: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);
    let effective_interface_name = effective_interface_name(&contract);
    let exports_mod_ident =
        config.internal_exports_mod_ident(&effective_interface_name);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    // Collect methods and check for entrypoint ID collisions
    let methods: Vec<_> = contract.methods_sorted().into_iter().collect();
    check_entrypoint_collisions(iface_name, &iface_vers, &methods)?;

    let mut exports = TokenStream::new();
    for method in &methods {
        exports.extend(generate_method(
            iface_name,
            Some(&contract),
            method,
            config,
        )?);
    }
    let entrypoints_wrapper = if contract.trait_path().is_none() {
        generate_entrypoints_wrapper(
            &effective_interface_name,
            iface_name,
            &iface_vers,
            &methods,
        )
    } else {
        TokenStream::new()
    };

    // For trait impls, emit compile-time assertions that the impl-side
    // entrypoint metadata matches the trait definition.
    let trait_metadata_assertions = if let (Some(trait_path), Some(self_ty)) =
        (contract.trait_path(), contract.self_ty())
    {
        let iface_name = contract
            .interface_name()
            .expect("trait impls always resolve an interface name");
        let iface_major =
            contract_util::semver::major_version_from_string(&iface_vers);
        let iface_name_prefix = "Interface name mismatch: impl-side name is ";
        let iface_name_suffix = ". Repeat #[contract(interface = ...)] on the impl to match the trait definition.";
        let iface_name_suffix_lit =
            LitByteStr::new(iface_name_suffix.as_bytes(), Span::call_site());
        let iface_name_suffix_len = iface_name_suffix.len();
        // Build the diagnostic in const-eval via sdk::internal::const_iface_version_mismatch_diagnostic_bytes,
        // then panic! with str::from_utf8.
        let prefix = format!(
            "Major version mismatch: the major version specified in #[contract(version = ...)] \
             is '{iface_major}' (from version '{iface_vers}'), but it does not match the trait's "
        );
        let suffix = " (if not specified: requested version defaults to the current crate's \
             version). Check the trait definition and Cargo.toml to see the trait's major version.";
        let suffix_lit = LitByteStr::new(suffix.as_bytes(), Span::call_site());
        let suffix_len = suffix.len();
        quote! {
            const _: () = {
                const TRAIT_INTERFACE: &str =
                    <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME;
                const PREFIX: &str = #iface_name_prefix;
                const SUFFIX: &[u8; #iface_name_suffix_len] =
                    &#iface_name_suffix_lit;
                const FULL: [u8; {
                    PREFIX.len() + 27 + #iface_name.len() + TRAIT_INTERFACE.len() + SUFFIX.len()
                }] = ::sdk::internal::const_iface_name_mismatch_diagnostic_bytes::<
                    { PREFIX.len() },
                    { #iface_name.len() },
                    { TRAIT_INTERFACE.len() },
                    { SUFFIX.len() },
                    {
                        PREFIX.len() + 27 + #iface_name.len() + TRAIT_INTERFACE.len()
                            + SUFFIX.len()
                    },
                >(PREFIX, #iface_name, TRAIT_INTERFACE, SUFFIX);

                if !::sdk::internal::const_str_eq(#iface_name, TRAIT_INTERFACE) {
                    panic!("{}", unsafe { core::str::from_utf8_unchecked(&FULL) });
                }
            };

            const _: () = {
                const TRAIT_MAJOR: &str =
                    <#self_ty as #trait_path>::__GROUNDHOG_IFACE_MAJOR_VERSION;
                const TRAIT_FULL_VERSION: &str =
                    <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION;
                const PREFIX: &str = #prefix;

                const SUFFIX: &[u8; #suffix_len] = &#suffix_lit;

                const FULL: [u8; {
                    PREFIX.len() + 11 + TRAIT_MAJOR.len() + TRAIT_FULL_VERSION.len() + SUFFIX.len()
                }] = ::sdk::internal::const_iface_version_mismatch_diagnostic_bytes::<
                    { PREFIX.len() },
                    { TRAIT_MAJOR.len() },
                    { TRAIT_FULL_VERSION.len() },
                    { SUFFIX.len() },
                    {
                        PREFIX.len() + 11 + TRAIT_MAJOR.len() + TRAIT_FULL_VERSION.len()
                            + SUFFIX.len()
                    },
                >(PREFIX, TRAIT_MAJOR, TRAIT_FULL_VERSION, SUFFIX);

                if !::sdk::internal::const_str_eq(#iface_major, TRAIT_MAJOR) {
                    panic!("{}", unsafe { core::str::from_utf8_unchecked(&FULL) });
                }
            };
        }
    } else {
        quote! {}
    };

    // The assertions go outside the module so the associated consts resolve
    // through the trait.
    Ok(quote! {
        #entrypoints_wrapper

        #[allow(non_snake_case)]
        mod #exports_mod_ident {
            use super::*;

            #exports
        }

        #trait_metadata_assertions
    })
}

pub fn generate_entrypoints(
    contract: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);
    let effective_interface_name = effective_interface_name(&contract);
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();
    let methods: Vec<_> = contract.methods_sorted().into_iter().collect();

    Ok(generate_entrypoints_wrapper(
        &effective_interface_name,
        iface_name,
        &iface_vers,
        &methods,
    ))
}

fn generate_entrypoints_wrapper(
    effective_interface_name: &str,
    iface_name: Option<&str>,
    iface_vers: &str,
    methods: &[impl Method],
) -> TokenStream {
    let entrypoints_ident =
        format_ident!("{}Entrypoints", effective_interface_name);
    let entrypoints = methods.iter().map(|method| {
        let ident = &method.sig().ident;
        let entrypoint =
            contract_entrypoint_id(iface_name, iface_vers, method.sig());
        let entrypoint_lit = entrypoint_literal(entrypoint);

        quote! {
            pub const #ident: Self = Self(#entrypoint_lit);
        }
    });

    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct #entrypoints_ident(::sdk::contract::ContractEntrypointID);

        impl From<#entrypoints_ident> for ::sdk::contract::ContractEntrypointID {
            fn from(value: #entrypoints_ident) -> Self {
                value.0
            }
        }

        #[allow(non_upper_case_globals)]
        impl #entrypoints_ident {
            pub fn into_u64(self) -> u64 {
                self.0.into_u64()
            }

            #(#entrypoints)*
        }
    }
}

/// Check for entrypoint ID collisions among methods
fn check_entrypoint_collisions(
    iface_name: Option<&str>,
    iface_vers: &str,
    methods: &[impl Method],
) -> Result<()> {
    let mut seen: HashMap<ContractEntrypointID, String> = HashMap::new();

    for method in methods {
        let sig = method.sig();
        let entrypoint = contract_entrypoint_id(iface_name, iface_vers, sig);
        let method_name = sig.ident.to_string();

        if let Some(existing) = seen.get(&entrypoint) {
            return Err(error(
                sig,
                format!(
                    "Entrypoint ID collision: methods '{}' and '{}' produce the same ID {:016X}",
                    existing,
                    method_name,
                    u64::from_be_bytes(*entrypoint)
                ),
            ));
        }
        seen.insert(entrypoint, method_name);
    }

    Ok(())
}

// Generate an exported function for:
// - An impl or impl-trait method with self-parameter
// - An impl or impl-trait associated function
// - A free function
pub fn generate_method(
    iface_name: Option<&str>,
    contract: Option<&impl ContractSrc>,
    method: &impl Method,
    config: &Config,
) -> Result<TokenStream> {
    let sig = method.sig();
    let ident = &sig.ident;
    let iface_vers = if let Some(contract) = contract {
        contract.defaulted_interface_vers()
    } else {
        config
            .version
            .clone()
            .unwrap_or_else(crate::contract::interface_vers_default)
    };

    let entrypoint_sig =
        contract_util::EntrypointSignature::new(iface_name, &iface_vers, sig);
    let entrypoint = entrypoint_sig.id();

    let export_name = entrypoint_export_name(entrypoint);
    let export_docstr = format!(" {} = {}", export_name, entrypoint_sig);

    let extern_name = format_ident!("{}_extern", ident);
    let internal_name = contract
        .map(|c| c.make_internal_name(ident))
        .unwrap_or_else(|| format_ident!("{}_internal", ident));
    // Is the function a method that takes a self-parameter?
    let is_method = contract.is_some() && method.sig().receiver().is_some();

    // Does the method take a mutable self-parameter?
    let is_mut = is_method
        && method
            .sig()
            .receiver()
            .as_ref()
            .map(|r| r.mutability.is_some())
            .unwrap_or(false);

    let arg_names = method_arg_name_list(method.sig());

    // Load the stored contract state (when appropriate)
    let load_state = if is_method {
        let self_ty = &contract.unwrap().self_ty();
        let mutability = if is_mut {
            quote! { mut }
        } else {
            quote! {}
        };

        let deref = if is_mut {
            quote! { state_node.deref_mut() }
        } else {
            quote! { state_node.deref() }
        };

        if config.storage() {
            // Construct contract state from keypaths
            quote! {
                use ::sdk::storage::{Keypath, Node};
                use core::ops::{Deref, DerefMut};
                let keypath = Keypath::root();
                let #mutability state_node: Node<#self_ty> = keypath.into();
                let #mutability state :&#mutability #self_ty = #deref;
            }
        } else {
            // If not using storage, we just create a default object to call the method on
            // TODO: if !config.storage(), assert presence of Default impl
            quote! {
                let #mutability state_default = <#self_ty as Default>::default();
                let #mutability state :&#mutability #self_ty = &#mutability state_default;
            }
        }
    } else {
        // Free function: no state variable at all
        quote! {}
    };

    // How to call the function
    let invoke = if is_method {
        // Invoke the method on a state object
        if let Some(contract) = contract {
            if let Some(trait_path) = contract.trait_path() {
                let self_ty = contract.self_ty();
                quote! { <#self_ty as #trait_path>::#ident(state, #arg_names) }
            } else {
                quote! { state.#ident(#arg_names) }
            }
        } else {
            unreachable!("is_method implies contract source")
        }
    } else if let Some(contract) = contract {
        // Invoke the associated function on the contract type
        let self_ty = &contract.self_ty();
        if let Some(trait_path) = contract.trait_path() {
            quote! { <#self_ty as #trait_path>::#ident(#arg_names) }
        } else {
            quote! { #self_ty::#ident(#arg_names) }
        }
    } else {
        // Invoke the free function
        quote! { #ident(#arg_names) }
    };

    let log_guest_call = if cfg!(feature = "logging") {
        let print_string = format!("[sdk] Guest: Calling method {}", ident);
        quote! { ::sdk::debug_log!("{} ", #print_string); }
    } else {
        quote! {}
    };

    // Decode the args, run the body, encode the result
    let wrapped_call = quote! {{
        //
        // If debug mode: log the method-call
        #log_guest_call

        let return_buf ={

        // Load contract state
        #load_state

        // Fetch the calldata from the VM
        let calldata = ::sdk::contract_op::get_calldata();

        // Decode the args
        let (#arg_names) =
            ::sdk::call::internal::contract_call_decode(calldata.as_ref())
                .unwrap_or_else(|e|
                    ::sdk::exn::decoding_args_throw(e));

        // TODO: can we get the fetch-and-decode into one internal function?

        // Call the method implementation (passing the contract state, if any,
        // and the args)
        let result = #invoke;

        // Encode the result
        ::sdk::call::internal::encode_success(&result)

        // Make sure any storage nodes are fully dropped (and written to environment).
        // Not technically necessary, given that the storage is wrapped in a Node<T>.
        };

        // Return the result to the VM
        ::sdk::contract_return(return_buf.as_ref());
    }};

    // Include method info in debugging context (when enabled)
    let wrapped_call = {
        let method_name = ident.to_string();
        let iface = match iface_name {
            Some(iface) => quote! { Some(#iface) },
            None => quote! { None },
        };
        let call_info = quote! {
            ::sdk::internal::HandlingMethod::new(#iface, #iface_vers, #method_name)
        };

        quote! {
            ::sdk::with_call_tracking!(#call_info, #wrapped_call)
        }
    };

    // Inline the internal implementation, unless requested otherwise
    let inline = if !cfg!(feature = "no_inline_methods") {
        quote! { #[inline(always)] }
    } else {
        quote! {}
    };

    // Wasm exports are only emitted when the "entrypoints" feature is
    // enabled.  The build script passes --features entrypoints; deps
    // that don't enable the feature cannot inject entrypoints.
    Ok(quote! {

        #[doc=#export_docstr]
        #[cfg(feature = "entrypoints")]
        #[cfg_attr(
            target_family = "wasm",
            unsafe(export_name = #export_name)
        )]
        pub extern "C" fn #extern_name() {
            #internal_name()
        }

        #inline
        #[allow(non_snake_case)]
        pub fn #internal_name() {
            #wrapped_call
        }
    })
}

pub fn generate_free_function_bundle(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let effective_interface_name = config
        .interface_name()
        .unwrap_or_else(|| "_unnamed_interface".to_string());
    let ident = &item.sig.ident;
    // Free functions remain one-off exports: they do not generate a shared
    // client or Entrypoints wrapper. Their hidden exports module therefore
    // includes the function name by default so multiple one-offs can share an
    // interface without colliding, but an explicit `exports = "..."`
    // override still wins when the caller wants to name that module.
    let exports_mod_ident = if config.exports.is_some() {
        config.internal_exports_mod_ident(&effective_interface_name)
    } else {
        free_function_exports_mod_ident(&effective_interface_name, ident)
    };
    let iface_name = config.interface_name();
    let iface_name = iface_name.as_deref();
    let contract: Option<&syn::ItemImpl> = None;
    let exports = generate_method(iface_name, contract, item, config)?;

    Ok(quote! {
        #[allow(non_snake_case)]
        mod #exports_mod_ident {
            use super::*;

            #exports
        }
    })
}

pub(crate) fn free_function_exports_mod_ident(
    effective_interface_name: &str,
    ident: &syn::Ident,
) -> syn::Ident {
    syn::Ident::new(
        &format!("__groundhog_exports_{}_{}", effective_interface_name, ident),
        Span::call_site(),
    )
}

#[cfg(test)]
mod tests {
    use super::free_function_exports_mod_ident;
    use super::*;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn free_function_hidden_exports_include_interface_and_method_name() {
        let config = parse_config(r#"interface = "Iface""#);
        let ident = syn::Ident::new("ping", Span::call_site());

        assert_eq!(
            free_function_exports_mod_ident(
                &config.interface_name().unwrap(),
                &ident,
            )
            .to_string(),
            "__groundhog_exports_Iface_ping"
        );
    }

    #[test]
    fn free_function_hidden_exports_honor_explicit_override() {
        let config =
            parse_config(r#"exports = "custom", interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        assert!(generated.contains("mod custom"));
    }
}
