//! Generate client-stub code from an `impl ContractSrc` and a `Config`

use super::config::{Config, ThrownType};
use super::contract::{ConfiguredContract, ContractSrc, Method};
use super::entrypoint::*;
use super::util::*;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::parse::Result;
use syn::spanned::Spanned;

pub const CLIENT_TRY_METHOD_PREFIX: &str = "try_";

pub const CLIENT_CONSTRUCTOR_NAME: &str = "new";
pub const CLIENT_GAS_LIMIT_HELPER_NAME: &str = "with_gas_limit";
pub const CLIENT_TRY_INVOKE_HELPER_NAME: &str = "with_commit_locking";

const CLIENT_RESERVED_NAMES: [&str; 3] = [
    CLIENT_CONSTRUCTOR_NAME,
    CLIENT_GAS_LIMIT_HELPER_NAME,
    CLIENT_TRY_INVOKE_HELPER_NAME,
];

pub fn reserved_client_name_error(name: &str) -> Option<String> {
    for reserved_name in CLIENT_RESERVED_NAMES {
        if name == reserved_name {
            return Some(format!(
                "the name `{}` is reserved by generated client helpers; rename \
                the contract item so client APIs stay consistent across traits, \
                impls, and free functions",
                name,
            ));
        }
    }

    if name.starts_with(CLIENT_TRY_METHOD_PREFIX) {
        return Some(format!(
            "names beginning with `{}` are reserved by generated client \
            inspection methods",
            CLIENT_TRY_METHOD_PREFIX,
        ));
    }

    None
}

fn client_constructor_ident() -> syn::Ident {
    format_ident!("{}", CLIENT_CONSTRUCTOR_NAME)
}

fn client_gas_limit_helper_ident() -> syn::Ident {
    format_ident!("{}", CLIENT_GAS_LIMIT_HELPER_NAME)
}

fn client_try_invoke_helper_ident() -> syn::Ident {
    format_ident!("{}", CLIENT_TRY_INVOKE_HELPER_NAME)
}

fn try_client_method_ident(method_ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}{}", CLIENT_TRY_METHOD_PREFIX, method_ident)
}

pub fn generate(
    contract: &(impl ContractSrc + Spanned),
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);

    let client_ident = contract.client_ident().ok_or_else(|| {
        error(
            &contract.span(),
            "contract client identifier could not be auto-derived \
                 (self type is not a simple path); \
                 provide 'client = ClientName' attribute",
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
    let constructor_ident = client_constructor_ident();
    let gas_limit_helper_ident = client_gas_limit_helper_ident();
    let try_invoke_helper_ident = client_try_invoke_helper_ident();

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
            use ::sdk::OrReThrow;

            impl #client_ident {
                pub fn #constructor_ident(address: &ContractAddress) -> Self {
                    Self {
                        address: address.clone(),
                        config: ::sdk::invoke_config::InvokeConfig::new()
                    }
                }

                pub fn #gas_limit_helper_ident(&self, gas_limit: u64) -> Self {
                    Self {
                        address: self.address.clone(),
                        config: ::sdk::invoke_config::InvokeConfig {
                            gas_limit,
                            ..self.config
                        },
                    }
                }

                pub fn #try_invoke_helper_ident(self) -> Self {
                    Self {
                        config: ::sdk::invoke_config::InvokeConfig {
                                flags: self.config.flags
                                    | ::sdk::invoke_config::InvokeFlag::TryInvoke.0,
                                .. self.config
                        },
                        .. self
                    }
                }

                #(#method_stubs)*

                pub const INTERFACE_VERSION: &str = #iface_vers;
            }

            impl ::sdk::call::ContractClient for #client_ident {
                fn new(address: &::sdk::ContractAddress) -> Self {
                    #client_ident::#constructor_ident(address)
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
            "contract client identifier could not be auto-derived \
                 (self type is not a simple path); \
                 provide 'client = ClientName' attribute",
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
            ::sdk::debug_log!("{} with args {:?}", #print_string, args);
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

    let try_ident = try_client_method_ident(ident);

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
        pub fn #ident(&self, #(#sent_params),* ) #sig_output
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                ::sdk::call::try_call_raw::<_, #return_type>(&self.address, entrypoint, self.config.gas_limit, &args)
                    .or_re_throw()
            })
        }

        // This method-wrapper returns a Result with the per-client error enum
        // so that exceptions may be inspected.
        #[allow(clippy::result_large_err)]
        #track_caller
        pub fn #try_ident(&self, #(#sent_params),* ) -> ::core::result::Result<#return_type, #error_enum_ident>
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                use ::sdk::call::RawCallError;
                match ::sdk::call::try_call_raw(&self.address, entrypoint, self.config.gas_limit, &args) {
                    Ok(val) => Ok(val),
                    Err(RawCallError::Fault(s)) => Err(#error_enum_ident::Fault(s)),
                    Err(RawCallError::Exception(ref exn)) => {
                        let selector = exn.selector();
                        let payload = exn.payload();
                        #selector_dispatch
                    }
                    Err(RawCallError::Invalid) => Err(#error_enum_ident::Invalid),
                    Err(RawCallError::UnparseableReturn(s)) => Err(#error_enum_ident::UnparseableReturn(s)),
                    Err(RawCallError::UnparseableAbort(s)) => Err(#error_enum_ident::UnparseableAbort(s)),
                    Err(RawCallError::UnparseableAuthHandles(s)) => Err(#error_enum_ident::UnparseableAuthHandles(s)),
                    Err(RawCallError::EncodingArgs(s)) => Err(#error_enum_ident::EncodingArgs(s)),
                }
            })
        }
    })
}

fn generate_error_enum(
    client_ident: &syn::Ident,
    throws_list: &[ThrownType],
) -> TokenStream {
    let error_enum_ident = format_ident!("{}Error", client_ident);

    let declared_variants: Vec<TokenStream> = throws_list
        .iter()
        .map(|thrown| {
            let variant = thrown.variant();
            let ty = thrown.ty();
            quote! { #variant(#ty) }
        })
        .collect();

    let display_declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|thrown| {
            let variant = thrown.variant();
            let ident_str = variant.to_string();
            quote! { Self::#variant(e) => ::core::write!(f, "{}({:?})", #ident_str, e) }
        })
        .collect();

    let rethrow_declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|thrown| {
            let variant = thrown.variant();
            quote! { Self::#variant(e) => ::sdk::throw(e) }
        })
        .collect();

    quote! {
        #[derive(Debug, PartialEq)]
        pub enum #error_enum_ident {
            #(#declared_variants,)*
            Library(::sdk::exn::LibraryExn),
            Fault(::sdk::exn::FaultString),
            Unknown(::sdk::exn::RawException),
            Invalid,
            BadDecode(::sdk::exn::RawException),
            UnparseableReturn(::sdk::exn::FaultString),
            UnparseableAbort(::sdk::exn::FaultString),
            UnparseableAuthHandles(::sdk::exn::FaultString),
            EncodingArgs(::sdk::exn::FaultString),
        }

        #[allow(unexpected_cfgs)]
        #[cfg(feature = "dev")]
        impl ::core::fmt::Display for #error_enum_ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self {
                    #(#display_declared_arms,)*
                    Self::Library(e) => ::core::write!(f, "SDK exception: {}", e),
                    Self::BadDecode(raw) => ::core::write!(f, "BadDecode({:?})", raw),
                    Self::Fault(e) => ::core::write!(f, "Contract faulted: {}", e),
                    Self::Unknown(e) => ::core::write!(f, "Unknown({:?})", e),
                    Self::Invalid => ::core::write!(f, "Invalid call: contract not found (or invalid auth tokens)"),
                    Self::UnparseableReturn(e) => ::core::write!(f, "UnparseableReturn({:?})", e),
                    Self::UnparseableAbort(e) => ::core::write!(f, "UnparseableAbort({:?})", e),
                    Self::UnparseableAuthHandles(e) => ::core::write!(f, "UnparseableAuthHandles({:?})", e),
                    Self::EncodingArgs(e) => ::core::write!(f, "EncodingArgs({:?})", e),
                }
            }
        }

        impl ::sdk::ReThrow for #error_enum_ident {
            fn re_throw(self) -> ! {
                use sdk::CallingError;
                use sdk::client_library_throw as cthrow;

                match self {
                    #(#rethrow_declared_arms,)*
                    Self::Library(e) => ::sdk::throw(e),
                    Self::BadDecode(raw) => ::sdk::ReThrow::re_throw(raw),
                    Self::Unknown(raw) => ::sdk::ReThrow::re_throw(raw),
                    Self::Fault(f) => panic!("{f}"),
                    Self::Invalid => cthrow(CallingError::Invalid),
                    Self::UnparseableReturn(s) => cthrow(CallingError::DecodingSuccess(s)),
                    Self::UnparseableAbort(s) => cthrow(CallingError::DecodingException(s)),
                    Self::UnparseableAuthHandles(s) => cthrow(CallingError::DecodingSuccess(s)),
                    Self::EncodingArgs(s) => cthrow(CallingError::EncodingArgs(s)),
                }
            }
        }
    }
}

fn generate_selector_dispatch(
    error_enum_ident: &syn::Ident,
    throws_list: &[ThrownType],
) -> TokenStream {
    let declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|thrown| {
            let variant = thrown.variant();
            let ty = thrown.ty();
            quote! {
                <#ty as ::sdk::exn::Exn>::SELECTOR => {
                    let err = ::sdk::call::internal::contract_call_decode_exn_prefix(payload);
                    if err.is_err() {
                        return Err(#error_enum_ident::BadDecode(
                            ::sdk::exn::RawException::new(selector, payload)));
                    }
                    Err(#error_enum_ident::#variant(err.unwrap()))
                }
            }
        })
        .collect();

    quote! {
        match selector {
            #(#declared_arms)*
            <::sdk::exn::LibraryExn as ::sdk::exn::Exn>::SELECTOR => {
                let err = ::sdk::call::internal::contract_call_decode_exn_prefix(payload);
                if err.is_err() {
                    return Err(#error_enum_ident::BadDecode(
                        ::sdk::exn::RawException::new(selector, payload)));
                }
                Err(#error_enum_ident::Library(err.unwrap()))
            }
            _ => {
                Err(#error_enum_ident::Unknown(
                    ::sdk::exn::RawException::new(selector, payload)))
            }
        }
    }
}
