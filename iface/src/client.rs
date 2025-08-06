//! Generate client-stub code from an `impl ContractSrc` and a `Config`

use super::contract::{ContractSrc, ConfiguredContract};
use super::config::Config;
use super::entrypoint::*;
use super::util::*;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::Result;
use syn::spanned::Spanned;

pub fn generate(
    contract: &(impl ContractSrc + Spanned),
    config: &Config,
) -> Result<TokenStream> {

    let contract = ConfiguredContract::new(config.clone(), contract);

    let client_ident = contract.client_ident().ok_or_else(||
            error(&contract.span(),
                "contract client identifier must be given \
                 by 'client = ClientName' attribute"))?;

    let mut method_stubs = TokenStream::new();
    for (index, method) in contract.methods_sorted().into_iter().enumerate() {
        method_stubs.extend(generate_client_fn(index, method, &contract)?);
    }

    let client: TokenStream = quote! {
        pub struct #client_ident {
            address: ::sdk::types::ContractAddress,
            config: ::sdk::invoke_config::InvokeConfig
        }

        const _: () = {
            use ::sdk::types::ContractAddress;
            use ::sdk::types::ContractEntrypointID;
            use ::sdk::types::RuntimeMemoryLength;
            use ::sdk::internal::{client_invoke, invoke, InvokeResult};
            use ::sdk::client_error::ClientResult;

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

                #method_stubs
            }
        };
    };

    Ok(client)
}

fn generate_client_fn(
    index: usize,
    method: &syn::Signature,
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_ref().map(|s| s.as_str());

    let version = None;
    let entrypoint = contract_entrypoint_id(
        iface_name,
        version,
        &method.ident,
        index
    );

    let ident = &method.ident;
    let mut params = method.inputs.iter().peekable();

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

    let print_string = format!("[sdk] Client: Calling method {}", ident);

    let log_call = quote! { 
        ::sdk::debug_log!("{} with args {:?}", #print_string, #args_tuple); 
    };

    let explicit_type = if let syn::ReturnType::Type(_, ty) = &method.output {
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
            .and_then(|t| generic_type_first_arg(&*t))
            .ok_or_else(||
                error(&method.output,
                    "A method using #[contract(err_type = E)] (or contained \
                     in a trait or impl using that attribute) must return \
                     type Result<_>"))?;
        quote! { ClientResult<#inner, #err_type> }
    } else {
        quote! { InvokeResult<#return_type> }
    };

    let invoke_fn = if contract.err_type().is_some() {
        quote! { client_invoke }
    } else {
        quote! { invoke }
    };

    Ok(quote_spanned! {method.span()=>

        pub fn #ident(&mut self, #(#sent_params),* ) -> #return_type
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            #invoke_fn(&self.address, entrypoint, self.config.gas_limit, &args)
        }

    })
}
