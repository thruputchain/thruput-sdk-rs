//! Generate exports

use super::Config;
use super::contract::{ConfiguredContract, ContractSrc};
use super::entrypoint::*;
use super::util::*;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::parse::Result;

/// Generate exports
pub fn generate(
    contract: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);

    let exports_mod_ident = config.exports_mod_ident();

    let self_ty = contract.self_ty();
    let mut uses = vec![quote! {
        use super::#self_ty;
    }];
    if let Some(trait_path) = contract.trait_path() {
        uses.push(quote! { use super::#trait_path; });
    }

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();

    let mut exports = TokenStream::new();
    for (index, method) in contract.methods_sorted().into_iter().enumerate() {
        exports.extend(generate_method(
            index,
            iface_name,
            Some(&contract),
            method,
            config,
        )?);
    }

    Ok(quote! {
        pub mod #exports_mod_ident {
            #(#uses)*

            #exports
        }
    })
}

// Generate an exported function for:
// - An impl or impl-trait method with self-parameter
// - An impl or impl-trait associated function
// - A free function
pub fn generate_method(
    index: usize,
    iface_name: Option<&str>,
    contract: Option<&impl ContractSrc>,
    method: &syn::Signature,
    config: &Config,
) -> Result<TokenStream> {
    let ident = &method.ident;
    let version = if let Some(contract) = contract {
        contract.defaulted_interface_vers()
    } else {
        "0.0.0".to_string()
    };

    let entrypoint =
        contract_entrypoint_id(iface_name, &version, ident, index);

    let export_name = entrypoint_export_name(entrypoint);
    let extern_name = format_ident!("{}_extern", ident);
    let internal_name = format_ident!("{}_internal", ident);
    let export_num_name =
        format_ident!("{}_EXPORT_NUM", ident.to_string().to_uppercase());

    let mut uses = vec![quote! {
        use ::sdk::contract::ContractEntrypointID;
        use ::sdk::contract_panic;
        use ::sdk::contract_op::{get_calldata, return_result, contract_abort};
        use ::sdk::internal::decode_xdr;
    }];

    let log_guest_call = if cfg!(feature = "logging") {
        uses.push(quote! { use ::sdk::debug_log; });

        let print_string = format!("[sdk] Guest: Calling method {}", ident);

        quote! { debug_log!("{} ", #print_string); }
    } else {
        quote! {}
    };

    let mut export = quote! {
        #[doc=#export_name]
        #[cfg_attr(target_family = "wasm", unsafe(export_name = #export_name))]
        pub extern "C" fn #extern_name() -> () {
            #internal_name()
        }
    };

    // The function is a method that takes a self-parameter
    let is_method = contract.is_some() && method.receiver().is_some();

    // The function is associated with a type
    let is_associated = contract.is_some();

    // How to call the function
    let function_or_method = if is_method {
        // Invoke the method on a state object
        quote! { state.#ident }
    } else if is_associated {
        // Invoke the associated function on the contract type
        let self_ty = &contract.unwrap().self_ty();
        quote! { #self_ty::#ident }
    } else {
        // Invoke the free function
        quote! { #ident }
    };

    // The method takes a mutable self-parameter
    let is_mut = is_method
        && method
            .receiver()
            .as_ref()
            .map(|r| r.mutability.is_some())
            .unwrap_or(false);

    // The method is marked "init": storage should be uninitialized before calling
    // TODO: use a method attribute to mark initialization methods
    let is_init = is_method && ident.to_string().as_str() == "initialize";

    let assert_uninitialized = if is_method && config.storage() && is_init {
        let self_ty = &contract.unwrap().self_ty();
        quote! {
            // Assert that the storage does not yet exist
            if <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                .unwrap_or_else(|_| contract_panic!("Failed loading contract storage")).is_some()
            {
                contract_panic!("#[contract(init)] method called when contract storage already exists");
            }
        }
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
            uses.push(quote! {
                use ::sdk::storage::{Keypath, LoadKeypath, DefaultFromKeypath, Store};
            });

            if is_init {
                // Get a default value to initialize in this method
                quote! {
                    let #mutability state =
                        <#self_ty as DefaultFromKeypath>::default_from_keypath(&Keypath::root());
                }
            } else {
                // Load the previously-stored value
                quote! {
                    let #mutability state =
                        <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                            .unwrap_or_else(|_| contract_panic!("Failed loading contract storage"))
                            .unwrap_or_else(|| contract_panic!("Contract storage not present"));
                }
            }
        } else {
            // If not using storage, we just create a default object to call the method on
            quote! {
                let #mutability state = <#self_ty as Default>::default();
            }
        }
    } else {
        // Free function: no state variable at all
        quote! {}
    };

    // Save the contract state after calling the method, if it takes a mutable ref to the state
    // TODO: use an attribute to request no saving
    let save_state = if is_method && config.storage() && is_mut {
        quote! {
            state.store_keypath(&Keypath::root()).unwrap_or_else(|_|
                contract_panic!("Failed saving contract storage"));
        }
    } else {
        quote! {}
    };

    let has_err_type = contract.and_then(|c| c.err_type()).is_some()
        // generate_method() might be called directly, so check config too
        || config.err_type().is_some();

    let unwrap_abort = if has_err_type {
        quote! {
            let result = result.unwrap_or_else(|e|
                contract_abort(&e));
        }
    } else {
        quote! {}
    };

    let arg_names = method_arg_name_list(method);

    let inline = if !cfg!(feature = "no_inline_methods") {
        quote! { #[inline(always)] }
    } else {
        quote! {}
    };

    export.extend(quote! {
        #inline
        pub fn #internal_name() -> () {
            #(#uses)*

            // If debug mode: log the method-call
            #log_guest_call

            // Error if this is an "init" method and the contract state already exists
            #assert_uninitialized

            #load_state

            let result = {

                // Fetch the calldata from the VM
                let calldata = get_calldata().unwrap_or_else(|e|
                    contract_panic!("get_calldata() failed: {}", e));

                // Decode the args
                let (#arg_names) = decode_xdr(calldata.as_ref()).unwrap_or_else(|e|
                    contract_panic!("failed decoding contract-call arguments: {}", e));

                // Call the free function or contract-type method
                let result = #function_or_method(#arg_names);

                result
            };

            // Unwrap a Result<T, E> (if requested by #[contract(error = E)],
            // and call contract_abort in the error case
            #unwrap_abort

            // Save any mutations
            #save_state

            // Return the contract-call result to the VM, ending execution of this contract
            return_result(&result);

            // Not reached
        }
    });

    // A hack for test setup
    if contract.is_none() {
        let entrypoint_lit = entrypoint_literal(entrypoint);

        export.extend(quote! {
            pub const #export_num_name: ::sdk::contract::ContractEntrypointID = #entrypoint_lit;
        });
    }

    Ok(export)
}
