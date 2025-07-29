//! Generate client-stub code from an `impl ContractSrc` and a `Config`

use super::contract::ContractSrc;
use super::config::Config;
use super::entrypoint::*;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{Error, Result};
use syn::spanned::Spanned;
use core::cmp::Ordering;

pub fn generate(
    src: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {

    let client_ident = config.client_ident()
        .or_else(|| src.client_ident())
        .ok_or_else(||
            error(&src.span(),
                "contract client identifier must be given \
                 by 'client = ClientName' attribute"))?;

    let mut methods: Vec<_> = src.methods().collect();
    methods.sort_by(cmp_method_ref_by_name);

    let mut method_stubs = TokenStream::new();
    for (index, method) in methods.iter().enumerate() {
        method_stubs.extend(generate_client_fn(index, method, src)?);
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
    let iface_ident = src.interface_ident();

    let version = None;
    let entrypoint = contract_entrypoint_id(
        iface_ident,
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

    let export_name = entrypoint_export_name(entrypoint);
    let entrypoint_lit = entrypoint_literal(entrypoint);

    let comment_text = format!("/// Export as {}", export_name);
    let comment_tokens: proc_macro2::TokenStream =
        comment_text.parse().unwrap();

    let print_string = format!("[sdk] Client: Calling method {}", ident);

    let log_call = quote! { 
        ::sdk::debug_log!("{} with args {:?}", #print_string, #args_tuple); 
    };

    Ok(quote_spanned! {method.span()=>

        pub fn #ident(&mut self, #(#sent_params),* ) #return_type
        {
            #comment_tokens
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            invoke(&self.address, entrypoint, #gas_limit, &args)
        }

    })
}

fn cmp_method_ref_by_name(a: &&syn::Signature, b: &&syn::Signature) -> Ordering {
    cmp_method_by_name(*a, *b)
}

fn cmp_method_by_name(a: &syn::Signature, b: &syn::Signature) -> Ordering {
    cmp_ident(&a.ident, &b.ident)
}

fn cmp_ident(a: &syn::Ident, b: &syn::Ident) -> Ordering {
    a.to_string().cmp(&b.to_string())
}

fn is_receiver(fn_arg: &syn::FnArg) -> bool {
    matches!(fn_arg, syn::FnArg::Receiver(_))
}

fn arg_pat(fn_arg: &syn::FnArg) -> Option<&syn::Pat> {
    match fn_arg {
        syn::FnArg::Typed(pat_type) => Some(&pat_type.pat),
        syn::FnArg::Receiver(_) => None,
    }
}

// Produce an error referencing a particular code span
fn error<S, M>(spanned: &S, msg: M) -> Error
where
    S: syn::spanned::Spanned,
    M: AsRef<str>,
{
    Error::new(spanned.span(), msg.as_ref())
}
