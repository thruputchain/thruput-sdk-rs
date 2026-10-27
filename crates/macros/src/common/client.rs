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
    let remote = if cfg!(feature = "remote") {
        generate_remote(&contract, &client_ident, &mut used)?
    } else {
        TokenStream::new()
    };

    let constructor_ident = client_constructor_ident();
    let gas_limit_helper_ident = client_gas_limit_helper_ident();
    let try_invoke_helper_ident = client_try_invoke_helper_ident();
    let read_only_helper_ident = client_read_only_helper_ident();

    let client = quote! {

        pub struct #client_ident {
            address: ::thruput_sdk::ContractAddress,
            config: ::thruput_sdk::InvokeConfig
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
                        config: ::thruput_sdk::InvokeConfig::new()
                    }
                }

                /// A copy of this client whose calls may spend at most
                /// `gas_limit` gas. The original client is unchanged.
                pub fn #gas_limit_helper_ident(&self, gas_limit: u64) -> Self {
                    Self {
                        address: self.address.clone(),
                        config: ::thruput_sdk::InvokeConfig {
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
                        config: ::thruput_sdk::InvokeConfig {
                                flags: self.config.flags
                                    | ::thruput_sdk::InvokeFlag::TryInvoke.0.cast_unsigned(),
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
                        config: ::thruput_sdk::InvokeConfig {
                                flags: self.config.flags
                                    | ::thruput_sdk::InvokeFlag::ReadOnly.0.cast_unsigned(),
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
                config: ::thruput_sdk::InvokeConfig,
                on_exn: #handler_ident,
            }

            impl ::thruput_sdk::MapExceptions for #client_ident {
                type Mapped<F> = #mapped_client_ident<F>;

                fn map_exceptions<X, F>(&self, on_exn: F) -> Self::Mapped<F>
                where
                    F: for<'s> ::core::ops::FnOnce(::thruput_sdk::exn::RawExceptionRef<'s>) -> X,
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

        #remote
    };

    Ok(client)
}

/// `FooClient` -> `FooRemote`; any other client name gains a `Remote` suffix.
fn remote_ident(client_ident: &syn::Ident) -> syn::Ident {
    let name = client_ident.to_string();
    let base = name.strip_suffix("Client").unwrap_or(&name);
    format_ident!("{}Remote", base)
}

/// Generate `<Iface>Remote<I>`, an off-chain proxy whose methods prepare
/// invocations through an `I: RemoteInvoker`. Declared `pure` and `readOnly`
/// methods become views; all others, including undeclared ones, are calls.
/// Arguments are sent as the guest client sends them.
fn generate_remote(
    contract: &impl ContractSrc,
    client_ident: &syn::Ident,
    used: &mut HashSet<String>,
) -> Result<TokenStream> {
    let remote_ident = remote_ident(client_ident);
    let invoker_ident = fresh_ident("__ThruputInvoker", used);
    let iface_name = contract.interface_name();
    let iface_vers = contract.defaulted_interface_vers();
    let interface = match iface_name.as_deref() {
        Some(name) => quote! { ::core::option::Option::Some(#name) },
        None => quote! { ::core::option::Option::None },
    };

    let mut method_names = HashSet::new();
    let mut stubs = Vec::new();
    for method in contract.methods_sorted() {
        let sig = method.sig();
        let ident = &sig.ident;
        let name = ident.to_string().trim_start_matches("r#").to_owned();
        method_names.insert(name.clone());

        let entrypoint = entrypoint_literal(contract_entrypoint_id(
            iface_name.as_deref(),
            &iface_vers,
            sig,
        ));
        let sent_params: Vec<&syn::FnArg> = sig
            .inputs
            .iter()
            .filter(|param| !is_receiver(param))
            .collect();
        let sent_args: Vec<&syn::Pat> =
            sent_params.iter().cloned().filter_map(arg_pat).collect();
        let args_tuple = if sent_args.len() == 1 {
            let arg = sent_args[0];
            quote! { #arg }
        } else {
            quote! { ( #(#sent_args),* ) }
        };
        let return_type = match &sig.output {
            syn::ReturnType::Type(_, ty) => quote! { #ty },
            syn::ReturnType::Default => quote! { () },
        };

        let effect = super::contract::method_effect_ident(&method)?;
        let read_only = effect
            .as_ref()
            .is_some_and(|effect| effect == "Pure" || effect == "ReadOnly");
        let (prepared, verb) = if read_only {
            (format_ident!("View"), format_ident!("view"))
        } else {
            (format_ident!("Call"), format_ident!("call"))
        };
        let effect = match effect {
            Some(effect) => quote! {
                ::core::option::Option::Some(::thruput_sdk::remote::MethodEffect::#effect)
            },
            None => quote! { ::core::option::Option::None },
        };
        let docs = method
            .attrs()
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));

        stubs.push(quote_spanned! {sig.span()=>
            #(#docs)*
            pub fn #ident(&self, #(#sent_params),*)
                -> <#invoker_ident as ::thruput_sdk::remote::RemoteInvoker>::#prepared<#return_type>
            {
                let __thruput_args = #args_tuple;
                ::thruput_sdk::remote::RemoteInvoker::#verb::<#return_type, _>(
                    &self.invoker,
                    &self.address,
                    ::thruput_sdk::remote::RemoteMethod {
                        interface: #interface,
                        version: #iface_vers,
                        name: #name,
                        entrypoint: #entrypoint,
                        effect: #effect,
                    },
                    &__thruput_args,
                )
            }
        });
    }

    // Interface methods take precedence over these accessors.
    let address_accessor = (!method_names.contains("address")).then(|| {
        quote! {
            /// The contract this proxy invokes.
            pub fn address(&self) -> ::thruput_sdk::ContractAddress {
                self.address.clone()
            }
        }
    });
    let invoker_accessor = (!method_names.contains("invoker")).then(|| {
        quote! {
            /// The invoker that prepares this proxy's invocations.
            pub fn invoker(&self) -> &#invoker_ident {
                &self.invoker
            }
        }
    });

    let doc = format!(
        "Off-chain proxy for the interface of [`{client_ident}`]. Each method \
         prepares an invocation through a \
         [`RemoteInvoker`](::thruput_sdk::remote::RemoteInvoker)."
    );
    Ok(quote! {
        #[doc = #doc]
        #[derive(Clone, Debug)]
        pub struct #remote_ident<#invoker_ident> {
            invoker: #invoker_ident,
            address: ::thruput_sdk::ContractAddress,
        }

        const _: () = {
            impl<#invoker_ident> #remote_ident<#invoker_ident> {
                /// A proxy for the contract at `address`, invoked through `invoker`.
                pub fn new(
                    invoker: #invoker_ident,
                    address: &::thruput_sdk::ContractAddress,
                ) -> Self {
                    Self {
                        invoker,
                        address: address.clone(),
                    }
                }

                #address_accessor
                #invoker_accessor
            }

            #[allow(dead_code)] // Consumers need not use every method.
            impl<#invoker_ident: ::thruput_sdk::remote::RemoteInvoker>
                #remote_ident<#invoker_ident>
            {
                #(#stubs)*
            }
        };
    })
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
            ::thruput_sdk::debug_log!("{} with args {:?}", #print_string, __thruput_args);
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

    let call_body = |exception_type: TokenStream,
                     on_exn: TokenStream,
                     result_suffix: TokenStream| {
        quote_spanned! {method.sig().span()=>
            let __thruput_entrypoint = #entrypoint_lit;
            let __thruput_args = #args_tuple;
            #log_call
            with_call_tracking!(#call_info, {
                ::thruput_sdk::call::try_call_raw_with::<_, #return_type, #exception_type>(
                    &self.address, __thruput_entrypoint, self.config.gas_limit,
                    &__thruput_args, self.config.flags, #on_exn,
                )
                #result_suffix
            })
        }
    };
    let throwing_body = call_body(
        quote_spanned! {method.sig().span()=> ::core::convert::Infallible },
        quote_spanned! {method.sig().span()=> |exn| ::thruput_sdk::ReThrow::re_throw(exn) },
        quote_spanned! {method.sig().span()=> .or_re_throw() },
    );
    let owned_body = call_body(
        quote_spanned! {method.sig().span()=> ::thruput_sdk::exn::RawException },
        quote_spanned! {method.sig().span()=> |exn| exn.to_owned() },
        quote_spanned! {method.sig().span()=>},
    );
    let mapped_body = call_body(
        quote_spanned! {method.sig().span()=> #exception_ident },
        quote_spanned! {method.sig().span()=> self.on_exn },
        quote_spanned! {method.sig().span()=>},
    );

    let ordinary = quote_spanned! {method.sig().span()=>
        #[cfg_attr(test, track_caller)]
        pub fn #ident(&self, #(#sent_params),* ) #sig_output {
            #throwing_body
        }

        /// An `Err` may hold a caught exception, kept untyped as an owned
        /// [`RawException`](::thruput_sdk::exn::RawException) — see that
        /// type's docs for size/ownership, and
        /// [`match_exn!`](::thruput_sdk::match_exn) for matching it against
        /// the exception types you care about, whatever this interface
        /// declares it `throws`.
        #[allow(clippy::result_large_err)]
        #[cfg_attr(test, track_caller)]
        pub fn #try_ident(&self, #(#sent_params),* ) -> ::core::result::Result<#return_type, ::thruput_sdk::call::CallError> {
            #owned_body
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
            #handler_ident: for<'s> ::core::ops::FnOnce(::thruput_sdk::exn::RawExceptionRef<'s>) -> #exception_ident,
        {
            #mapped_body
        }
    };
    Ok((ordinary, mapped))
}
