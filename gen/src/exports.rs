//! Generate exports

use super::Config;
use super::contract::{ConfiguredContract, ContractSrc, Method};
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
    trait_name: Option<&str>,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);

    let exports_mod_ident = config.exports_mod_ident_for_trait(trait_name);

    let mut uses = vec![quote! {
        use super::*;
    }];
    if let Some(trait_path) = contract.trait_path() {
        uses.push(quote! { use super::#trait_path; });
    }

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    // Collect methods and check for entrypoint ID collisions
    let methods: Vec<_> = contract.methods_sorted().into_iter().collect();
    check_entrypoint_collisions(iface_name, &iface_vers, &methods)?;

    let mut exports = TokenStream::new();
    for method in methods {
        exports.extend(generate_method(
            iface_name,
            Some(&contract),
            &method,
            config,
        )?);
    }

    // For trait impls, emit a compile-time assertion that the major version used
    // for entrypoint IDs matches the major version baked into the trait definition.
    let version_assertion = if let (Some(trait_path), Some(self_ty)) =
        (contract.trait_path(), contract.self_ty())
    {
        let iface_major =
            contract_util::semver::major_version_from_string(&iface_vers);
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

    // The version assertion goes outside the module so the associated const
    // resolves through the trait.
    Ok(quote! {
        #[allow(non_snake_case)]
        pub mod #exports_mod_ident {
            #(#uses)*

            #exports
        }

        #version_assertion
    })
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
    let export_num_name =
        format_ident!("{}_EXPORT_NUM", ident.to_string().to_uppercase());

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

    // How to call the function
    let invoke = if is_method {
        // Invoke the method on a state object
        if let Some(trait_path) = contract.and_then(|c| c.trait_path()) {
            if is_mut {
                quote! { #trait_path::#ident(&mut state, #arg_names) }
            } else {
                quote! { #trait_path::#ident(&state, #arg_names) }
            }
        } else {
            quote! { state.#ident(#arg_names) }
        }
    } else if let Some(contract) = contract {
        // Invoke the associated function on the contract type
        // XXX: as above
        let self_ty = &contract.self_ty();
        quote! { #self_ty::#ident(#arg_names) }
    } else {
        // Invoke the free function
        quote! { #ident(#arg_names) }
    };

    // Is the method marked #[init]?
    let is_init = is_method && method.config().init.is_present();
    // TODO: complain about #[init] in inappropriate positions

    // If #[init], storage must be uninitialized before calling
    let assert_uninitialized = if is_method && config.storage() && is_init {
        let self_ty = &contract.unwrap().self_ty();
        quote! {{
            use ::sdk::storage::{Keypath, LoadKeypath};

            // Assert that the storage does not yet exist
            if <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                .unwrap_or_else(|_|
                    panic!("Failed loading contract storage")).is_some() // XX abort
            {
                panic!("\"init\" method called when contract storage already exists"); // XX abort
            }
        }}
    } else {
        quote! {}
    };

    // Load the stored contract state (when appropriate)
    let load_state = if is_method {
        let self_ty = &contract.unwrap().self_ty();
        let mutability = if is_mut {
            quote! { mut }
        } else {
            quote! {}
        };

        if config.storage() {
            if is_init {
                // Get a default value to initialize in this method
                quote! {
                    use ::sdk::storage::{Keypath, DefaultFromKeypath};
                    let keypath = Keypath::root();
                    let #mutability state =
                        <#self_ty as DefaultFromKeypath>::default_from_keypath(&keypath);
                }
            } else {
                // Load the previously-stored value
                quote! {
                    use ::sdk::storage::{Keypath, LoadKeypath};
                    let keypath = Keypath::root();
                    let #mutability state =
                        <#self_ty as LoadKeypath>::try_load_keypath(&keypath)
                            .unwrap_or_else(|_| panic!("Failed loading contract storage"))
                            .unwrap_or_else(|| panic!("Contract storage not present"));
                }
            }
        } else {
            // If not using storage, we just create a default object to call the method on
            // TODO: if !config.storage(), assert presence of Default impl
            quote! {
                let #mutability state = <#self_ty as Default>::default();
            }
        }
    } else {
        // Free function: no state variable at all
        quote! {}
    };

    // If the method takes a mutable ref to the contract state, save the state
    // afterwards:
    let save_state = if is_method && config.storage() && is_mut {
        quote! {
            use ::sdk::storage::Store;
            state.store_keypath(&keypath);
        }
    } else {
        quote! {}
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

        // Abort if this is an "init" method and the contract state already
        // exists
        #assert_uninitialized

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
        let return_buf = ::sdk::call::internal::encode_success(&result);

        // Save any mutations
        #save_state

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

    // Export the entrypoint ID as a constant for use in tests
    let entrypoint_lit = entrypoint_literal(entrypoint);

    // Inline the internal implementation, unless requested otherwise
    let inline = if !cfg!(feature = "no_inline_methods") {
        quote! { #[inline(always)] }
    } else {
        quote! {}
    };

    Ok(quote! {

        #[doc=#export_docstr]
        #[cfg_attr(target_family = "wasm", unsafe(export_name = #export_name))]
        pub extern "C" fn #extern_name() {
            #internal_name()
        }

        #inline
        #[allow(non_snake_case)]
        pub fn #internal_name() {
            #wrapped_call
        }

        pub const #export_num_name: ::sdk::contract::ContractEntrypointID =
            #entrypoint_lit;

    })
}
