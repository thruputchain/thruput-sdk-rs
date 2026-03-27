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

    let throws_list = contract.throws_list();
    let error_enum = generate_error_enum(&client_ident, &throws_list);

    let define_spec_fn = if cfg!(feature = "spec") {
        generate_spec_fn(&contract, config)?
    } else {
        quote! {}
    };

    let method_stubs = contract
        .methods_sorted()
        .into_iter()
        .map(|method| generate_client_fn(&method, &contract))
        .collect::<Result<Vec<TokenStream>>>()?;

    let iface_vers = contract.defaulted_interface_vers();

    let client = quote! {

        #define_spec_fn

        #error_enum

        pub struct #client_ident {
            address: ::sdk::ContractAddress,
            config: ::sdk::invoke_config::InvokeConfig
        }

        const _: () = {
            use ::sdk::{ContractAddress, RuntimeMemoryLength};
            use ::sdk::contract::ContractEntrypointID;
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

                pub const INTERFACE_VERSION: &str = #iface_vers;
            }

            impl ::sdk::call::ContractClient for #client_ident {
                fn new(address: &::sdk::ContractAddress) -> Self {
                    #client_ident::new(address)
                }
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
    method: &impl Method,
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    let sig = method.sig();
    let ident = &sig.ident;
    let sig_output = &sig.output;

    let entrypoint = contract_entrypoint_id(iface_name, &iface_vers, sig);

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

    let call_ident = ident.to_string();
    let call_iface = match iface_name {
        Some(iface) => quote! { Some(#iface) },
        None => quote! { None },
    };
    let call_info = quote! {
        CallMethod::new(#call_iface, #iface_vers, #call_ident)
    };

    let track_caller = if cfg!(feature = "call_tracking") {
        quote! { #[track_caller] }
    } else {
        quote! {}
    };

    let try_ident = format_ident!("try_{}", ident);

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

    let client_ident = contract
        .client_ident()
        .unwrap_or_else(|| format_ident!("UnknownClient"));
    let error_enum_ident = format_ident!("{}Error", client_ident);
    let throws_list = contract.throws_list();
    let selector_dispatch =
        generate_selector_dispatch(&error_enum_ident, &throws_list);

    Ok(quote_spanned! {method.sig().span()=>

        // This method-wrapper re-throws if the called contract throws.
        #track_caller
        pub fn #ident(&mut self, #(#sent_params),* ) #sig_output
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                match ::sdk::call::try_call_raw::<_, #return_type>(&self.address, entrypoint, self.config.gas_limit, &args) {
                    Ok(val) => val,
                    Err(::sdk::call::RawCallError::Exception { selector, ref payload }) => {
                        ::sdk::call::re_throw(selector, payload.as_slice())
                    }
                    Err(::sdk::call::RawCallError::Fault(f)) => {
                        panic!("{}", f)
                    }
                }
            })
        }

        // This method-wrapper returns a Result with the per-client error enum
        // so that exceptions may be inspected.
        #[allow(clippy::result_large_err)]
        #track_caller
        pub fn #try_ident(&mut self, #(#sent_params),* ) -> ::core::result::Result<#return_type, #error_enum_ident>
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                match ::sdk::call::try_call_raw(&self.address, entrypoint, self.config.gas_limit, &args) {
                    Ok(val) => Ok(val),
                    Err(::sdk::call::RawCallError::Fault(s)) => Err(#error_enum_ident::Fault(s)),
                    Err(::sdk::call::RawCallError::Exception { selector, ref payload }) => {
                        let payload = payload.as_slice();
                        #selector_dispatch
                    }
                }
            })
        }
    })
}

fn generate_error_enum(
    client_ident: &syn::Ident,
    throws_list: &[&syn::Ident],
) -> TokenStream {
    const RESERVED: &[&str] = &["Library", "Fault", "Unknown"];
    for ident in throws_list {
        if RESERVED.contains(&ident.to_string().as_str()) {
            return syn::Error::new(
                ident.span(),
                format!(
                    "`{}` is a reserved error variant name \
                     (used by the generated client enum)",
                    ident
                ),
            )
            .to_compile_error();
        }
    }

    let error_enum_ident = format_ident!("{}Error", client_ident);

    let declared_variants: Vec<TokenStream> = throws_list
        .iter()
        .map(|ident| quote! { #ident(#ident) })
        .collect();

    let display_declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|ident| {
            quote! { Self::#ident(e) => ::core::write!(f, "{}", e) }
        })
        .collect();

    quote! {
        #[derive(Debug, PartialEq)]
        pub enum #error_enum_ident {
            #(#declared_variants,)*
            Library(::sdk::exn::LibraryExn),
            Fault(::sdk::exn::FaultString),
            Unknown(::sdk::exn::RawError),
        }

        #[allow(unexpected_cfgs)]
        #[cfg(feature = "dev")]
        impl ::core::fmt::Display for #error_enum_ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    #(#display_declared_arms,)*
                    Self::Library(e) => ::core::write!(f, "{}", e),
                    Self::Fault(e) => ::core::write!(f, "{}", e),
                    Self::Unknown(e) => ::core::write!(f, "{:?}", e),
                }
            }
        }
    }
}

fn generate_selector_dispatch(
    error_enum_ident: &syn::Ident,
    throws_list: &[&syn::Ident],
) -> TokenStream {
    let declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|ident| {
            quote! {
                <#ident as ::sdk::exn::Exn>::SELECTOR => {
                    let err = ::sdk::call::internal::contract_call_decode_prefix(payload)
                        .unwrap_or_else(|e|
                            ::sdk::exn::library_throw(
                                ::sdk::exn::CallingError::DecodingException(
                                    ::sdk::exn::FaultString::from_display(e))));
                    Err(#error_enum_ident::#ident(err))
                }
            }
        })
        .collect();

    quote! {
        match selector {
            #(#declared_arms)*
            <::sdk::exn::LibraryExn as ::sdk::exn::Exn>::SELECTOR => {
                let err = ::sdk::call::internal::contract_call_decode_prefix(payload)
                    .unwrap_or_else(|e|
                        ::sdk::exn::library_throw(
                            ::sdk::exn::CallingError::DecodingException(
                                ::sdk::exn::FaultString::from_display(e))));
                Err(#error_enum_ident::Library(err))
            }
            _ => {
                Err(#error_enum_ident::Unknown(
                    ::sdk::exn::RawError::new(selector, payload)))
            }
        }
    }
}
