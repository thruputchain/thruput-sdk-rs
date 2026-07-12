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

    let method_stubs = contract
        .methods_sorted()
        .into_iter()
        .map(|method| generate_client_fn(&method, &contract))
        .collect::<Result<Vec<TokenStream>>>()?;

    let iface_vers = contract.defaulted_interface_vers();
    let constructor_ident = client_constructor_ident();
    let gas_limit_helper_ident = client_gas_limit_helper_ident();
    let try_invoke_helper_ident = client_try_invoke_helper_ident();
    let read_only_helper_ident = client_read_only_helper_ident();

    let client = quote! {

        #error_enum

        pub struct #client_ident {
            address: ::sdk::ContractAddress,
            config: ::sdk::invoke_config::InvokeConfig
        }

        const _: () = {
            use ::sdk::{ContractAddress, RuntimeMemoryLength};
            use ::sdk::contract::ContractEntrypointID;
            use ::sdk::with_call_tracking;
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

                pub fn #read_only_helper_ident(self) -> Self {
                    Self {
                        config: ::sdk::invoke_config::InvokeConfig {
                                flags: self.config.flags
                                    | ::sdk::invoke_config::InvokeFlag::ReadOnly.0,
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
        ::sdk::internal::CallMethod::new(#call_iface, #iface_vers, #call_ident)
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
        #[cfg_attr(test, track_caller)]
        pub fn #ident(&self, #(#sent_params),* ) #sig_output
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                ::sdk::call::try_call_raw_with::<_, #return_type, ::core::convert::Infallible>(
                    &self.address, entrypoint, self.config.gas_limit, &args, self.config.flags,
                    |exn| exn.re_throw(),
                )
                .or_re_throw()
            })
        }

        // This method-wrapper returns a Result with the per-client error enum
        // so that exceptions may be inspected.
        ///
        /// An `Err` may hold a caught exception (the `Unknown` / `BadDecode`
        /// variants, each an owned [`RawException`](::sdk::exn::RawException)) —
        /// see that type's docs for size/ownership.
        #[allow(clippy::result_large_err)]
        #[cfg_attr(test, track_caller)]
        pub fn #try_ident(&self, #(#sent_params),* ) -> ::core::result::Result<#return_type, #error_enum_ident>
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                use ::sdk::call::RawCallError;
                match ::sdk::call::try_call_raw_with::<_, #return_type, #error_enum_ident>(
                    &self.address, entrypoint, self.config.gas_limit, &args, self.config.flags,
                    |exn| { #selector_dispatch },
                ) {
                    Ok(val) => Ok(val),
                    Err(RawCallError::Exception(e)) => Err(e),
                    Err(RawCallError::Fault(s)) => Err(#error_enum_ident::Fault(s)),
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
    let selector_uniqueness_asserts =
        generate_selector_uniqueness_asserts(throws_list);

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
            /// An exception whose selector doesn't match any declared throw
            /// type. Holds an owned [`RawException`](::sdk::exn::RawException)
            /// — see that type's docs for size/ownership.
            Unknown(::sdk::exn::RawException),
            Invalid,
            /// An exception matching a declared selector whose payload
            /// failed to decode. Holds an owned
            /// [`RawException`](::sdk::exn::RawException) — see that
            /// type's docs for size/ownership.
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
            // Unconditional on purpose: user crates don't declare a
            // `debugging` feature, so a cfg_attr would never fire there —
            // and the unread Location plumbing is fully eliminated in
            // optimized guest builds (measured: ±0 bytes across all
            // example contracts).
            #[track_caller]
            fn re_throw(self) -> ! {
                use sdk::CallingError;
                use sdk::client_library_throw as cthrow;

                match self {
                    #(#rethrow_declared_arms,)*
                    Self::Library(e) => cthrow(e),
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

        #selector_uniqueness_asserts
    }
}

/// Emit a compile-time `assert!` for every unordered pair drawn from the
/// declared `throws(...)` list plus the implicit `LibraryExn` arm, checking
/// that no two have the same [`Exn::SELECTOR`](::sdk::exn::Exn). Selectors
/// are derived from the exception type's own crate/name (not the path the
/// caller wrote), so two differently-spelled `throws(...)` entries (or a
/// user re-declaring `throws(sdk::exn::LibraryExn)`, aliasing the implicit
/// `Library` arm) can name the same underlying type without tripping the
/// name-based duplicate-variant check in [`ThrowsList::new`]. Left
/// unchecked, `generate_selector_dispatch` would emit two match arms with
/// the same constant selector pattern — silently dead code (or an
/// `unreachable_patterns` warning) rather than a clear build error.
/// `const _: () = assert!(...)` is evaluated at build time, so this catches
/// it up front. O(n^2) is fine: throws lists are tiny.
fn generate_selector_uniqueness_asserts(
    throws_list: &[ThrownType],
) -> TokenStream {
    let mut tys: Vec<TokenStream> = throws_list
        .iter()
        .map(|thrown| {
            let ty = thrown.ty();
            quote! { #ty }
        })
        .collect();
    tys.push(quote! { ::sdk::exn::LibraryExn });

    let mut asserts = Vec::with_capacity(tys.len() * tys.len() / 2);
    for i in 0..tys.len() {
        for j in (i + 1)..tys.len() {
            let ty_a = &tys[i];
            let ty_b = &tys[j];
            // The message literal is assembled at macro-expansion time, so
            // the const assert can name the colliding pair without any
            // const-context formatting.
            let msg = format!(
                "duplicate exception selector in throws(...) list: `{}` collides with `{}`",
                ty_a.to_string().replace(' ', ""),
                ty_b.to_string().replace(' ', ""),
            );
            asserts.push(quote! {
                const _: () = ::core::assert!(
                    <#ty_a as ::sdk::exn::Exn>::SELECTOR
                        != <#ty_b as ::sdk::exn::Exn>::SELECTOR,
                    #msg,
                );
            });
        }
    }

    quote! { #(#asserts)* }
}

fn generate_selector_dispatch(
    error_enum_ident: &syn::Ident,
    throws_list: &[ThrownType],
) -> TokenStream {
    let mut pairs: Vec<(TokenStream, TokenStream)> = throws_list
        .iter()
        .map(|thrown| {
            let variant = thrown.variant();
            let ty = thrown.ty();
            (quote! { #ty }, quote! { #variant })
        })
        .collect();
    pairs.push((quote! { ::sdk::exn::LibraryExn }, quote! { Library }));

    let arms: Vec<TokenStream> = pairs
        .iter()
        .map(|(ty, variant)| {
            quote! {
                <#ty as ::sdk::exn::Exn>::SELECTOR => {
                    match ::sdk::call::contract_call_decode_exn_prefix(exn.payload()) {
                        Ok(err) => #error_enum_ident::#variant(err),
                        Err(_) => #error_enum_ident::BadDecode(exn.to_owned()),
                    }
                }
            }
        })
        .collect();

    quote! {
        match exn.selector() {
            #(#arms)*
            _ => #error_enum_ident::Unknown(exn.to_owned()),
        }
    }
}
