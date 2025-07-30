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
            address: ::sdk::types::ContractAddress
        }

        const _: () = {
            use ::sdk::types::ContractAddress;
            use ::sdk::types::ContractEntrypointID;
            use ::sdk::types::RuntimeMemoryLength;
            use ::sdk::internal::invoke;

            impl #client_ident {
                pub fn new(address: &ContractAddress) -> Self {
                    Self { address: address.clone() }
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
    src: &impl ContractSrc,
) -> Result<TokenStream> {
    let iface_name = src.interface_name();
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

    let return_type = &method.output;
    let gas_limit = u64::MAX;

    let entrypoint_lit = entrypoint_literal(entrypoint);

    let print_string = format!("[sdk] Client: Calling method {}", ident);

    let log_call = quote! { 
        ::sdk::debug_log!("{} with args {:?}", #print_string, #args_tuple); 
    };

    Ok(quote_spanned! {method.span()=>

        pub fn #ident(&mut self, #(#sent_params),* ) #return_type
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;
            let gas_limit = #gas_limit;

            #log_call

            invoke(&self.address, entrypoint, gas_limit, &args)
        }

    })
}
