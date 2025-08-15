//! Generate exports

use super::entrypoint::*;
use super::util::*;
use super::contract::{ContractSrc, ConfiguredContract};
use super::Config;
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

    let self_ty = contract.self_ty();

    let exports_mod_ident = config.exports_mod_ident();

    let use_trait = if let Some(trait_path) = contract.trait_path() {
        quote! { use super::#trait_path; }
    } else {
        quote! {}
    };

    let use_storage = if config.storage() {
        quote! {
            use ::sdk::storage::{Keypath, LoadKeypath, DefaultFromKeypath, Store};
        }
    } else {
        quote! {}
    };

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_ref().map(|s| s.as_str());

    let mut exports = TokenStream::new();
    for (index, method) in contract.methods_sorted().into_iter().enumerate() {
        exports.extend(generate_method(index, iface_name, Some(&contract), method, config)?);
    }

    Ok(quote! {
        pub mod #exports_mod_ident {
            #use_trait
            use super::#self_ty;
            #use_storage
            use ::sdk::serialize::Serialize;
            use ::sdk::types::ContractEntrypointID;
            use ::sdk::contract_panic;
            use ::sdk::contract_op::contract_abort;

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
    let version = None;
    let entrypoint = contract_entrypoint_id(iface_name, version, ident, index);

    let export_name = entrypoint_export_name(entrypoint);
    let extern_name = format_ident!("{}_extern", ident);
    let internal_name = format_ident!("{}_internal", ident);
    let export_num_name =
        format_ident!("{}_EXPORT_NUM", ident.to_string().to_uppercase());

    let log_guest_call = if cfg!(feature = "logging") {
        let print_string = format!("[sdk] Guest: Calling method {}", ident);

        quote! {
            ::sdk::debug_log!("{} ", #print_string);
        }
    } else {
        quote! {}
    };

    let mut export = quote! {
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
    let is_mut = is_method && method.receiver().as_ref().map(|r| r.mutability.is_some()).unwrap_or(false);

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
        let mutability = if is_mut { quote! { mut } } else { quote! {} };

        if config.storage() {
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

    let use_alloc = if cfg!(feature = "alloc") {
        quote! {
            extern crate alloc;
            use alloc::boxed::Box;
        }
    } else { quote! {} };

    let has_err_type = contract.and_then(|c| c.err_type()).is_some()
        // generate_method() might be called directly, so check config too
        || config.err_type().is_some();

    let unwrap_abort = if has_err_type {
        quote! {
            let result = result.unwrap_or_else(|e|
                contract_abort(&e));
        }
    } else { quote! {} };

    let arg_names = method_arg_name_list(method);

    export.extend(quote! {
        pub fn #internal_name() -> () {
            use sdk::contract_op::{contract_return, get_calldata};
            use sdk::serialize::{encode_xdr, decode_xdr};
            #use_alloc

            // Debug mode: log the method-call
            #log_guest_call

            #assert_uninitialized

            #load_state

            // Get the encoded argument data from the VM
            let call_data: Box<[u8]> = get_calldata().unwrap_or_else(|_|
                sdk::contract_panic!("get_calldata() failed"));

            // Decode the arguments expected by the method
            let (#arg_names) = decode_xdr(call_data.as_ref()).unwrap_or_else(|_|
                sdk::contract_panic!("failed decoding contract-call arguments"));

            // Call the free function or contract-type method
            let result = #function_or_method(#arg_names);

            // Unwrap a Result<T, E> (if requested by #[contract(error = E)],
            // and call contract_abort in the error case
            #unwrap_abort

            // Save any mutations
            #save_state

            // Encode the result
            let result_bytes = encode_xdr(&result).unwrap_or_else(|_|
                sdk::contract_panic!("failed encoding contract-call result"));

            // Return the contract-call result to the VM
            contract_return(result_bytes.as_slice());

            // notreached
        }
    });

    // A hack for test setup
    if contract.is_none() {
        let entrypoint_lit = entrypoint_literal(entrypoint);

        export.extend(quote! {
            pub const #export_num_name : ContractEntrypointID = #entrypoint_lit;
        });
    }

    Ok(export)
}
