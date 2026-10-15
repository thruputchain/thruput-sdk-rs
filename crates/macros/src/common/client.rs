//! Generate client-stub code from an `impl ContractSrc` and a `Config`

use super::config::Config;
use super::contract::{ConfiguredContract, ContractSrc, Method};
use super::entrypoint::*;
use super::util::*;
use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use std::collections::HashSet;
use syn::parse::Result;
use syn::spanned::Spanned;

pub const CLIENT_TRY_METHOD_PREFIX: &str = "try_";

pub const CLIENT_CONSTRUCTOR_NAME: &str = "new";
pub const CLIENT_GAS_LIMIT_HELPER_NAME: &str = "with_gas_limit";
pub const CLIENT_TRY_INVOKE_HELPER_NAME: &str = "with_commit_locking";
pub const CLIENT_READ_ONLY_HELPER_NAME: &str = "with_read_only";

const CLIENT_RESERVED_NAMES: [&str; 4] = [
    CLIENT_CONSTRUCTOR_NAME,
    CLIENT_GAS_LIMIT_HELPER_NAME,
    CLIENT_TRY_INVOKE_HELPER_NAME,
    CLIENT_READ_ONLY_HELPER_NAME,
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

fn client_read_only_helper_ident() -> syn::Ident {
    format_ident!("{}", CLIENT_READ_ONLY_HELPER_NAME)
}

fn try_client_method_ident(method_ident: &syn::Ident) -> syn::Ident {
    format_ident!("{}{}", CLIENT_TRY_METHOD_PREFIX, method_ident)
}

// Proc-macro item/type identifiers are not hygienic. In particular an adapter's
// generic must not shadow a contract's argument or return type.
fn collect_idents(tokens: TokenStream, used: &mut HashSet<String>) {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => {
                used.insert(ident.to_string().trim_start_matches("r#").into());
            }
            TokenTree::Group(group) => collect_idents(group.stream(), used),
            _ => {}
        }
    }
}

fn fresh_ident(base: &str, used: &mut HashSet<String>) -> syn::Ident {
    let mut candidate = base.to_owned();
    while !used.insert(candidate.clone()) {
        candidate.push('_');
    }
    format_ident!("{}", candidate)
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

    let mut used = HashSet::from([client_ident
        .to_string()
        .trim_start_matches("r#")
        .into()]);
    for method in contract.methods() {
        collect_idents(method.sig().to_token_stream(), &mut used);
    }
    let mapped_client_ident = fresh_ident("__ThruputMappedClient", &mut used);
    let handler_ident = fresh_ident("__ThruputExceptionHandler", &mut used);
    let exception_ident = fresh_ident("__ThruputException", &mut used);

    let (method_stubs, mapped_method_stubs): (Vec<_>, Vec<_>) = contract
        .methods_sorted()
        .into_iter()
        .map(|method| {
            generate_client_fn(
                &method,
                &contract,
                &handler_ident,
                &exception_ident,
            )
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .unzip();

    let iface_vers = contract.defaulted_interface_vers();
    let interface_id_lit = interface_id_literal(
        contract.interface_name().as_deref(),
        &iface_vers,
    );
    let constructor_ident = client_constructor_ident();
    let gas_limit_helper_ident = client_gas_limit_helper_ident();
    let try_invoke_helper_ident = client_try_invoke_helper_ident();
    let read_only_helper_ident = client_read_only_helper_ident();

    let client = quote! {

        pub struct #client_ident {
            address: ::thruput_sdk::ContractAddress,
            config: ::thruput_sdk::invoke_config::InvokeConfig
        }

        const _: () = {
            use ::thruput_sdk::{ContractAddress, RuntimeMemoryLength};
            use ::thruput_sdk::contract::ContractEntrypointID;
            use ::thruput_sdk::with_call_tracking;
            use ::thruput_sdk::OrReThrow;

            impl #client_ident {
                /// A client for the contract deployed at `address`, with the
                /// default invocation configuration: no gas cap and no
                /// invocation flags.
                pub fn #constructor_ident(address: &ContractAddress) -> Self {
                    Self {
                        address: address.clone(),
                        config: ::thruput_sdk::invoke_config::InvokeConfig::new()
                    }
                }

                /// A copy of this client whose calls may spend at most
                /// `gas_limit` gas. The original client is unchanged.
                pub fn #gas_limit_helper_ident(&self, gas_limit: u64) -> Self {
                    Self {
                        address: self.address.clone(),
                        config: ::thruput_sdk::invoke_config::InvokeConfig {
                            gas_limit,
                            ..self.config
                        },
                    }
                }

                /// A copy of this client whose calls ask the callee frame to
                /// acquire its commit locks before returning, so a `try_`
                /// call observes a commit conflict as a failure instead of
                /// the whole transaction failing later. The original client
                /// is unchanged.
                pub fn #try_invoke_helper_ident(&self) -> Self {
                    Self {
                        address: self.address.clone(),
                        config: ::thruput_sdk::invoke_config::InvokeConfig {
                                flags: self.config.flags
                                    | ::thruput_sdk::invoke_config::InvokeFlag::TryInvoke.0.cast_unsigned(),
                                .. self.config
                        },
                    }
                }

                /// A copy of this client whose calls are rejected if the
                /// callee writes storage, deploys a contract, or performs any
                /// other state change. The original client is unchanged.
                pub fn #read_only_helper_ident(&self) -> Self {
                    Self {
                        address: self.address.clone(),
                        config: ::thruput_sdk::invoke_config::InvokeConfig {
                                flags: self.config.flags
                                    | ::thruput_sdk::invoke_config::InvokeFlag::ReadOnly.0.cast_unsigned(),
                                .. self.config
                        },
                    }
                }

                #(#method_stubs)*

                pub const INTERFACE_VERSION: &str = #iface_vers;
            }

            impl ::thruput_sdk::call::ContractClient for #client_ident {
                const INTERFACE_ID: ::thruput_sdk::InterfaceId = #interface_id_lit;

                fn new(address: &::thruput_sdk::ContractAddress) -> Self {
                    #client_ident::#constructor_ident(address)
                }
            }

            // Scoped to this client expansion: no new module-level type or
            // contract method names are reserved by the adapter.
            #[must_use = "the exception mapper is used by a generated try_* call"]
            pub struct #mapped_client_ident<#handler_ident> {
                address: ::thruput_sdk::ContractAddress,
                config: ::thruput_sdk::invoke_config::InvokeConfig,
                on_exn: #handler_ident,
            }

            impl ::thruput_sdk::MapExceptions for #client_ident {
                type Mapped<F> = #mapped_client_ident<F>;

                fn map_exceptions<X, F>(&self, on_exn: F) -> Self::Mapped<F>
                where
                    F: for<'s> FnOnce(::thruput_sdk::exn::RawExceptionRef<'s>) -> X,
                {
                    #mapped_client_ident {
                        address: self.address.clone(),
                        config: self.config,
                        on_exn,
                    }
                }
            }

            #[allow(dead_code)] // Consumers need not use every generated call style.
            impl<#handler_ident> #mapped_client_ident<#handler_ident> {
                #(#mapped_method_stubs)*
            }
        };
    };

    Ok(client)
}

fn generate_client_fn(
    method: &impl Method,
    contract: &impl ContractSrc,
    handler_ident: &syn::Ident,
    exception_ident: &syn::Ident,
) -> Result<(TokenStream, TokenStream)> {
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    let sig = method.sig();
    let ident = &sig.ident;
    let sig_output = &sig.output;

    let entrypoint = contract_entrypoint_id(iface_name, &iface_vers, sig);

    let mut params = method.sig().inputs.iter().peekable();

    // Pop the self parameter, if any
    if let Some(param) = params.peek()
        && is_receiver(param)
    {
        let _ = params.next().unwrap();
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
            ::thruput_sdk::debug_log!("{} with args {:?}", #print_string, __gh_args);
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
        ::thruput_sdk::internal::CallMethod::new(#call_iface, #iface_vers, #call_ident)
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

    let ordinary = quote_spanned! {method.sig().span()=>

        // This method-wrapper re-throws if the called contract throws.
        #[cfg_attr(test, track_caller)]
        pub fn #ident(&self, #(#sent_params),* ) #sig_output
        {
            let __gh_entrypoint = #entrypoint_lit;
            let __gh_args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                ::thruput_sdk::call::try_call_raw_with::<_, #return_type, ::core::convert::Infallible>(
                    &self.address, __gh_entrypoint, self.config.gas_limit, &__gh_args, self.config.flags,
                    |exn| ::thruput_sdk::ReThrow::re_throw(exn),
                )
                .or_re_throw()
            })
        }

        // This method-wrapper returns a Result so that exceptions may be
        // inspected.
        ///
        /// An `Err` may hold a caught exception, kept untyped as an owned
        /// [`RawException`](::thruput_sdk::exn::RawException) — see that
        /// type's docs for size/ownership, and
        /// [`match_exn!`](::thruput_sdk::match_exn) for matching it against
        /// the exception types you care about, whatever this interface
        /// declares it `throws`.
        #[allow(clippy::result_large_err)]
        #[cfg_attr(test, track_caller)]
        pub fn #try_ident(&self, #(#sent_params),* ) -> ::core::result::Result<#return_type, ::thruput_sdk::call::CallError>
        {
            let __gh_entrypoint = #entrypoint_lit;
            let __gh_args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                // The exception is copied out of the call scratch verbatim
                // and decoded on demand, rather than eagerly downcast to a
                // type this interface happens to declare.
                ::thruput_sdk::call::try_call_raw_with::<_, #return_type, ::thruput_sdk::exn::RawException>(
                    &self.address, __gh_entrypoint, self.config.gas_limit, &__gh_args, self.config.flags,
                    |exn| exn.to_owned(),
                )
            })
        }
    };

    let mapped = quote_spanned! {method.sig().span()=>
        /// Call once, mapping a borrowed exception before releasing the call scratch.
        /// See [`MapExceptions`](::thruput_sdk::MapExceptions).
        #[allow(clippy::result_large_err)]
        #[cfg_attr(test, track_caller)]
        pub fn #try_ident<#exception_ident>(self, #(#sent_params),*)
            -> ::core::result::Result<#return_type, ::thruput_sdk::call::RawCallError<#exception_ident>>
        where
            #handler_ident: for<'s> FnOnce(::thruput_sdk::exn::RawExceptionRef<'s>) -> #exception_ident,
        {
            let __gh_entrypoint = #entrypoint_lit;
            let __gh_args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                ::thruput_sdk::call::try_call_raw_with::<_, #return_type, #exception_ident>(
                    &self.address, __gh_entrypoint, self.config.gas_limit,
                    &__gh_args, self.config.flags, self.on_exn,
                )
            })
        }
    };
    Ok((ordinary, mapped))
}
