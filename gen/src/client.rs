//! Generate client-stub code from an `impl ContractSrc` and a `Config`

use super::config::Config;
use super::contract::{ConfiguredContract, ContractSrc, Method};
use super::entrypoint::*;
use super::util::*;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::parse::Result;
use syn::spanned::Spanned;

pub fn generate(
    contract: &(impl ContractSrc + Spanned),
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);

    let client_ident = contract.client_ident().ok_or_else(|| {
        error(
            &contract.span(),
            "contract client identifier must be given \
                 by 'client = ClientName' attribute",
        )
    })?;

    let define_spec_fn = if cfg!(feature = "spec") {
        generate_spec_fn(&contract, config)?
    } else {
        quote! {}
    };

    let method_stubs = contract
        .methods_sorted()
        .into_iter()
        .enumerate()
        .map(|(index, method)| generate_client_fn(index, &method, &contract))
        .collect::<Result<Vec<TokenStream>>>()?;

    let client = quote! {

        #define_spec_fn

        pub struct #client_ident {
            address: ::sdk::ContractAddress,
            config: ::sdk::invoke_config::InvokeConfig
        }

        const _: () = {
            use ::sdk::{ContractAddress, RuntimeMemoryLength};
            use ::sdk::contract::ContractEntrypointID;
            use ::sdk::invoke::{invoke, InvokeResult, client_invoke, ClientResult};
            use ::sdk::with_call_tracking;
            use ::sdk::internal::CallMethod;

            impl #client_ident {
                pub fn new(address: &ContractAddress) -> Self {
                    Self {
                        address: address.clone(),
                        config: ::sdk::invoke_config::InvokeConfig::new()
                    }
                }

                pub fn with_gas_limit(&mut self, gas_limit: u64) -> &mut Self {
                    self.config.gas_limit = gas_limit;
                    self
                }

                #(#method_stubs)*
            }
        };
    };

    Ok(client)
}

fn generate_spec_fn(
    contract: &impl ContractSrc,
    _config: &Config,
) -> Result<TokenStream> {
    let client_ident = contract.client_ident().ok_or_else(|| {
        error(
            &contract.span(),
            "contract client identifier must be given \
                 by 'client = ClientName' attribute",
        )
    })?;

    #[allow(unused)]
    let spec_fn = format_ident!(
        "{}_spec",
        contract.interface_name().unwrap_or("contract".into())
    );
    let spec = crate::spec::contract_repr(contract)?;

    Ok(quote! {
        #[cfg(feature = "spec")] // contract-macro-user must define feature "spec"
        pub fn #spec_fn() -> ::sdk::spec::Contract {

            #spec
        }

        #[cfg(feature = "spec")]
        impl ::sdk::spec::HasContractSpec for #client_ident {
            fn contract_spec() -> ::sdk::spec::Contract {
                #spec_fn()
            }
        }
    })
}

fn generate_client_fn(
    index: usize,
    method: &impl Method,
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();
    let ident = &method.sig().ident;

    let entrypoint =
        contract_entrypoint_id(iface_name, &iface_vers, ident, index);

    let mut params = method.sig().inputs.iter().peekable();

    // Pop the self parameter, if any
    if let Some(param) = params.peek() {
        if is_receiver(param) {
            let _ = params.next().unwrap();
        }
    }

    let sent_params: Vec<&syn::FnArg> = params.collect();
    let sent_args: Vec<&syn::Pat> =
        sent_params.iter().cloned().filter_map(arg_pat).collect();
    let args_tuple = if sent_args.len() == 1 {
        let arg = sent_args[0];
        quote! { #arg }
    } else {
        quote! { ( #(#sent_args),* ) }
    };

    let entrypoint_lit = entrypoint_literal(entrypoint);

    let log_call = if cfg!(feature = "logging") {
        let print_string = format!("[sdk] Client: Calling method {}", ident);

        quote! {
            ::sdk::debug_log!("{} with args {:?}", #print_string, #args_tuple);
        }
    } else {
        quote! {}
    };

    let explicit_type =
        if let syn::ReturnType::Type(_, ty) = &method.sig().output {
            Some(ty)
        } else {
            None
        };

    let return_type = if let Some(ty) = explicit_type {
        quote! { #ty }
    } else {
        quote! { () }
    };

    // If using #[contract(err_type = E)], convert a provided Result<T, E>
    // (which must be written literally as Result<T, E> or Result<T>)
    // to ClientResult<T, E>
    let return_type = if let Some(err_type) = contract.err_type() {
        let inner: &syn::Type = explicit_type
            .and_then(|t| generic_type_first_arg(t))
            .ok_or_else(|| {
                error(
                    &method.sig().output,
                    "A method using #[contract(err_type = E)] (or contained \
                     in a trait or impl using that attribute) must return \
                     type Result<_>",
                )
            })?;
        quote! { ClientResult<#inner, #err_type> }
    } else {
        quote! { InvokeResult<#return_type> }
    };

    let call_ident = ident.to_string();
    let call_iface = match iface_name {
        Some(iface) => quote! { Some(#iface) },
        None => quote! { None },
    };
    let call_info = quote! {
        CallMethod::new(#call_iface, #iface_vers, #call_ident)
    };

    let invoke_fn = if contract.err_type().is_some() {
        quote! { client_invoke }
    } else {
        quote! { invoke }
    };

    let track_caller = if cfg!(feature = "call_tracking") {
        quote! { #[track_caller] }
    } else {
        quote! {}
    };

    Ok(quote_spanned! {method.sig().span()=>

        #track_caller
        pub fn #ident(&mut self, #(#sent_params),* ) -> #return_type
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                // with-abort-handler
                #invoke_fn(&self.address, entrypoint, self.config.gas_limit, &args)
            })
        }

    })
}
