//! Generate exports

use super::config::Config;
use super::contract::{
    ConfiguredContract, ContractSrc, Method, effective_interface_name,
};
use super::entrypoint::*;
use super::util::*;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use std::collections::HashMap;
use syn::LitByteStr;
use syn::parse::Result;
use syn::spanned::Spanned;
use thruput_contract_util as contract_util;
use thruput_xdr::contract::ContractEntrypointID;

fn hex_symbol_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn encoded_crate_scope(package_name: &str, package_version: &str) -> String {
    format!(
        "{}__{}",
        hex_symbol_component(package_name),
        hex_symbol_component(package_version)
    )
}

/// Generate exports
pub fn generate(
    contract: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);
    let effective_interface_name = effective_interface_name(&contract);
    let exports_mod_ident =
        config.internal_exports_mod_ident(&effective_interface_name);

    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();

    // Collect methods and check for entrypoint ID collisions
    let methods: Vec<_> = contract.methods_sorted().into_iter().collect();
    if contract.trait_path().is_none() {
        check_entrypoint_collisions(iface_name, &iface_vers, &methods)?;
    }

    let mut exports = TokenStream::new();
    for method in &methods {
        exports.extend(generate_method(&contract, method)?);
    }
    let entrypoints_wrapper = if contract.trait_path().is_none() {
        generate_entrypoints_wrapper(
            &effective_interface_name,
            iface_name,
            &iface_vers,
            &methods,
        )
    } else {
        TokenStream::new()
    };

    // One interface-table record per impl block (see
    // `thruput_sdk::interface_table`): the same `#[used]` + KEEP'd-section
    // shape as the entrypoint records, and gated like them on rv32 — a
    // dependency that does not enable `entrypoints` exports nothing, so it
    // must not advertise anything either.
    let interface_record = contract.self_ty().map(|_| {
        let id_lit = if let Some(trait_path) = contract.trait_path() {
            let self_ty = contract.self_ty();
            quote!(<#self_ty as #trait_path>::INTERFACE_ID)
        } else {
            interface_id_literal(iface_name, &iface_vers)
        };
        quote! {
            #[cfg(any(not(target_arch = "riscv32"), feature = "entrypoints"))]
            ::thruput_sdk::__interface_record!(#id_lit);
        }
    });

    // Resolve trait metadata and check an explicit interface-name assertion.
    let trait_metadata_assertions = if let (Some(trait_path), Some(self_ty)) =
        (contract.trait_path(), contract.self_ty())
    {
        let iface_name = contract
            .interface_name()
            .expect("trait impls always resolve an interface name");
        let check_name = config.interface.is_some();
        let iface_name_prefix = "Interface name mismatch: impl-side name is ";
        let iface_name_suffix = ". Omit the impl-side interface override to inherit the trait's name.";
        let iface_name_suffix_lit =
            LitByteStr::new(iface_name_suffix.as_bytes(), Span::call_site());
        let iface_name_suffix_len = iface_name_suffix.len();
        let name_assertion = check_name.then(|| quote! {
            const _: () = {
                const TRAIT_INTERFACE: &str =
                    <#self_ty as #trait_path>::__THRUPUT_IFACE_NAME;
                const PREFIX: &str = #iface_name_prefix;
                const SUFFIX: &[u8; #iface_name_suffix_len] =
                    &#iface_name_suffix_lit;
                const FULL: [u8; {
                    PREFIX.len() + 27 + #iface_name.len() + TRAIT_INTERFACE.len() + SUFFIX.len()
                }] = ::thruput_sdk::internal::const_iface_name_mismatch_diagnostic_bytes::<
                    { PREFIX.len() },
                    { #iface_name.len() },
                    { TRAIT_INTERFACE.len() },
                    { SUFFIX.len() },
                    {
                        PREFIX.len() + 27 + #iface_name.len() + TRAIT_INTERFACE.len()
                            + SUFFIX.len()
                    },
                >(PREFIX, #iface_name, TRAIT_INTERFACE, SUFFIX);

                if !::thruput_sdk::internal::const_str_eq(#iface_name, TRAIT_INTERFACE) {
                    panic!("{}", unsafe { core::str::from_utf8_unchecked(&FULL) });
                }
            };
        });
        quote! {
            // Resolve the inherited identity even when guest entrypoints are
            // disabled. This also keeps a private contract type live when
            // its generated entrypoint wrappers are all cfg'd out.
            const _: ::thruput_sdk::InterfaceId = <#self_ty as #trait_path>::INTERFACE_ID;
            #name_assertion
        }
    } else {
        quote! {}
    };

    // Emit a host-gated static that registers (self_ty::DOMAIN, "<module>::<self_ty>")
    // into STORAGE_DOMAINS so the collision-checker can detect two roots sharing a domain.
    // Gated cfg(not(target_arch = "riscv32")) to match the registry's own gate.
    // Each contract block gets a unique static name derived from the self_ty +
    // trait (or "inherent") to avoid symbol collisions across multiple impl blocks.
    //
    // Also emits a host-only #[test] (one per contract block) that calls
    // assert_unique_storage_domains() so any domain collision is caught by `cargo test`.
    //
    // Only emitted when the contract block actually roots state — i.e. at least
    // one method takes `self` (`&self`/`&mut self`). That is exactly the
    // condition under which `contract_state_root::<self_ty>()` (which carries
    // the `StorageDomain` bound) is invoked, so gating on it keeps the
    // registration's `StorageDomain` requirement always satisfiable. A
    // storageless contract (only associated functions, no `#[storage]`) roots
    // no state and so needs no registration.
    let has_state = methods.iter().any(|m| m.sig().receiver().is_some());
    let (domain_registration, domain_test) = if let (true, Some(self_ty)) =
        (has_state, contract.self_ty())
    {
        // Unique suffix per contract block: self_ty + trait path (or "inherent").
        // Used for both the registration static name and the test function name.
        let self_ty_str = quote!(#self_ty)
            .to_string()
            .replace(|c: char| !c.is_alphanumeric(), "_");
        let disc = match contract.trait_path() {
            Some(tp) => quote!(#tp)
                .to_string()
                .replace(|c: char| !c.is_alphanumeric(), "_"),
            None => "inherent".to_string(),
        };
        let reg_ident = format_ident!("__GH_DOMAIN_{}_{}", self_ty_str, disc);
        let test_ident = format_ident!(
            "__gh_storage_domains_unique_{}_{}",
            self_ty_str,
            disc
        );
        let registration = quote! {
            #[cfg(not(target_arch = "riscv32"))]
            #[::thruput_sdk::__linkme::distributed_slice(::thruput_sdk::storage::STORAGE_DOMAINS)]
            #[linkme(crate = ::thruput_sdk::__linkme)]
            #[allow(non_upper_case_globals)]
            static #reg_ident: ::thruput_sdk::storage::DomainRecord =
                ::thruput_sdk::storage::DomainRecord {
                    // The framed domain bytes themselves: the registry
                    // compares them for byte equality, which is exactly
                    // on-chain root collision (the framing is injective).
                    domain: <#self_ty as ::thruput_sdk::storage::StorageDomain>::DOMAIN,
                    type_path: concat!(module_path!(), "::", stringify!(#self_ty)),
                };
        };
        // NOTE: this is a `#[cfg(test)]` LIB test. A contract crate whose [lib]
        // sets `test = false` will silently NOT compile or run it, so that crate
        // gets no storage-domain collision detection (and no warning). Keep lib
        // tests enabled for the guarantee to hold.
        let test = quote! {
            #[cfg(all(test, not(target_arch = "riscv32")))]
            #[test]
            #[allow(non_snake_case)]
            fn #test_ident() {
                ::thruput_sdk::storage::assert_unique_storage_domains();
            }
        };
        (registration, test)
    } else {
        (quote! {}, quote! {})
    };

    // The assertions go outside the module so the associated consts resolve
    // through the trait.
    Ok(quote! {
        #entrypoints_wrapper

        #[allow(non_snake_case)]
        mod #exports_mod_ident {
            use super::*;

            #exports
        }

        #trait_metadata_assertions

        #interface_record

        #domain_registration

        #domain_test
    })
}

pub fn generate_entrypoints(
    contract: &impl ContractSrc,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);
    let effective_interface_name = effective_interface_name(&contract);
    let iface_name = contract.interface_name();
    let iface_name = iface_name.as_deref();
    let iface_vers = contract.defaulted_interface_vers();
    let methods: Vec<_> = contract.methods_sorted().into_iter().collect();

    Ok(generate_entrypoints_wrapper(
        &effective_interface_name,
        iface_name,
        &iface_vers,
        &methods,
    ))
}

fn generate_entrypoints_wrapper(
    effective_interface_name: &str,
    iface_name: Option<&str>,
    iface_vers: &str,
    methods: &[impl Method],
) -> TokenStream {
    let entrypoints_ident =
        format_ident!("{}Entrypoints", effective_interface_name);
    let interface_id_lit = interface_id_literal(iface_name, iface_vers);
    let entrypoints = methods.iter().map(|method| {
        let ident = &method.sig().ident;
        let entrypoint =
            contract_entrypoint_id(iface_name, iface_vers, method.sig());
        let entrypoint_lit = entrypoint_literal(entrypoint);

        quote! {
            pub const #ident: Self = Self(#entrypoint_lit);
        }
    });

    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct #entrypoints_ident(::thruput_sdk::contract::ContractEntrypointID);

        impl From<#entrypoints_ident> for ::thruput_sdk::contract::ContractEntrypointID {
            fn from(value: #entrypoints_ident) -> Self {
                value.0
            }
        }

        #[allow(non_upper_case_globals)]
        impl #entrypoints_ident {
            /// The identity of this interface at its major version, shared
            /// by every entrypoint below and derived by
            /// `thruput_contract_util::interface_id`. Equal to the trait's
            /// `INTERFACE_ID` for trait-defined interfaces.
            pub const INTERFACE_ID: ::thruput_sdk::InterfaceId = #interface_id_lit;

            pub fn into_u64(self) -> u64 {
                self.0.into_u64()
            }

            #(#entrypoints)*
        }
    }
}

/// Check for entrypoint ID collisions among methods
pub fn check_entrypoint_collisions(
    iface_name: Option<&str>,
    iface_vers: &str,
    methods: &[impl Method],
) -> Result<()> {
    let mut seen: HashMap<ContractEntrypointID, String> = HashMap::new();

    for method in methods {
        let sig = method.sig();
        let entrypoint = contract_entrypoint_id(iface_name, iface_vers, sig);
        let method_name = sig.ident.to_string();

        if let Some(existing) = seen.get(&entrypoint) {
            return Err(error(
                sig,
                format!(
                    "Entrypoint ID collision: methods '{}' and '{}' produce the same ID {:016X}",
                    existing,
                    method_name,
                    u64::from_be_bytes(*entrypoint)
                ),
            ));
        }
        seen.insert(entrypoint, method_name);
    }

    Ok(())
}

/// The method call's arguments: owned params pass through; `&T` params pass
/// `RefArg::get(&decoded)`.
fn method_call_args(sig: &syn::Signature) -> TokenStream {
    let args = method_args(sig).map(|(ident, ty)| match ty {
        // Spanned on the parameter type, so a type with no `RefArg` impl
        // is reported at the parameter rather than at the attribute.
        syn::Type::Reference(reference) if reference.mutability.is_none() => {
            let elem = &reference.elem;
            quote::quote_spanned!(elem.span()=>
                ::thruput_sdk::call::RefArg::get(&#ident)
            )
        }
        _ => quote!(#ident),
    });
    quote! { #( #args ),* }
}

// Generate an exported function for:
// - An impl or impl-trait method with self-parameter
// - An impl or impl-trait associated function
// - A free function
pub fn generate_method(
    contract: &impl ContractSrc,
    method: &impl Method,
) -> Result<TokenStream> {
    let sig = method.sig();
    let ident = &sig.ident;
    let self_ty = contract.self_ty();
    let trait_impl = contract.trait_path().zip(self_ty);
    let method_id_lit = method_id_tokens(contract, sig);
    let crate_scope = invoking_crate_scope();
    let (export_docstr, call_iface, call_version, guards) = if let Some((
        trait_path,
        self_ty,
    )) = &trait_impl
    {
        (
            format!("Entrypoint for trait method `{ident}`."),
            quote!(Some(<#self_ty as #trait_path>::__THRUPUT_IFACE_NAME)),
            quote!(<#self_ty as #trait_path>::__THRUPUT_IFACE_FULL_VERSION),
            quote!(#trait_path!(#ident, #crate_scope);),
        )
    } else {
        let iface_name = contract.interface_name();
        let iface_name = iface_name.as_deref();
        let iface_vers = contract.defaulted_interface_vers();
        let entrypoint_sig = contract_util::EntrypointSignature::new(
            iface_name,
            &iface_vers,
            sig,
        );
        let export_name = entrypoint_export_name(entrypoint_sig.id());
        let call_iface = match iface_name {
            Some(name) => quote!(Some(#name)),
            None => quote!(None),
        };
        (
            format!(" {} = {}", export_name, entrypoint_sig),
            call_iface,
            quote!(#iface_vers),
            generate_entrypoint_guards(
                sig,
                iface_name,
                &iface_vers,
                quote!(#crate_scope),
            ),
        )
    };

    let extern_name = format_ident!("{}_extern", ident);
    let internal_name = contract.make_internal_name(ident);
    let is_method = self_ty.is_some() && sig.receiver().is_some();

    // Does the method take a mutable self-parameter?
    let is_mut = is_method
        && method
            .sig()
            .receiver()
            .as_ref()
            .map(|r| {
                r.mutability.is_some()
                    || matches!(
                        &r.kind,
                        syn::ReceiverKind::Reference(_, _, Some(_))
                    )
            })
            .unwrap_or(false);

    let arg_names = method_arg_name_list(method.sig());
    let call_args = method_call_args(method.sig());

    // Load the stored contract state (when appropriate)
    let load_state = if is_method {
        let mutability = if is_mut {
            quote! { mut }
        } else {
            quote! {}
        };

        let deref = if is_mut {
            quote! { __gh_state_node.deref_mut() }
        } else {
            quote! { __gh_state_node.deref() }
        };

        quote! {
            use core::ops::{Deref, DerefMut};
            let #mutability __gh_state_node: ::thruput_sdk::storage::Node<#self_ty> =
                ::thruput_sdk::storage::contract_state_root::<#self_ty>();
            let #mutability __gh_state :&#mutability #self_ty = #deref;
        }
    } else {
        // Free function: no state variable at all
        quote! {}
    };

    // Trait methods always dispatch through the same resolved trait path.
    let invoke = if let Some((trait_path, self_ty)) = &trait_impl {
        let receiver = is_method.then(|| quote!(__gh_state,));
        quote!(<#self_ty as #trait_path>::#ident(#receiver #call_args))
    } else if is_method {
        quote!(__gh_state.#ident(#call_args))
    } else if let Some(self_ty) = self_ty {
        quote!(#self_ty::#ident(#call_args))
    } else {
        quote!(#ident(#call_args))
    };

    let log_guest_call = if cfg!(feature = "logging") {
        let print_string = format!("[sdk] Guest: Calling method {}", ident);
        quote! { ::thruput_sdk::debug_log!("{} ", #print_string); }
    } else {
        quote! {}
    };

    // Decode the args, run the body, encode the result. The calldata buffer
    // is a local of this (per-entrypoint) wrapper frame, filled in place by
    // the shared, non-generic `fetch_calldata_and_auth_tokens` — no by-value
    // copy of the no-alloc SCRATCH_MAX buffer — and the decoded args borrow
    // it for the whole frame. The fetch returns the number N of auth tokens
    // the caller supplied, which are this frame's auth token ids 0..N-1.
    //
    // Both calls here (wrapper -> fetch, and the handler body itself) are
    // direct. That is load-bearing for the chain's funcref-table cap (64
    // initial elements): passing the handler to the SDK as a closure or
    // `fn` pointer reifies one table slot per entrypoint, which pushed
    // many-entrypoint contracts past the cap. The shared machinery stays
    // one copy per contract because the fetch fn is non-generic and
    // `#[inline(never)]`.
    let wrapped_call = quote! {{
        //
        // If debug mode: log the method-call
        #log_guest_call

        let mut __gh_calldata: ::thruput_sdk::contract_op::Calldata =
            ::thruput_sdk::internal::new_scratch();
        let __gh_auth_token_count: u32 =
            ::thruput_sdk::contract_op::fetch_calldata_and_auth_tokens(
                &mut __gh_calldata,
            );

        let __gh_res = {

            // Load contract state
            #load_state

            // Decode the args (Signer<T> reads 0 bytes, gets sentinel handle;
            // contract_call_decode then fills in Signer handles from the auth
            // token ids 0..N-1, in order)
            let mut __args =
                ::thruput_sdk::call::contract_call_decode(
                    __gh_calldata.as_ref(),
                    0..__gh_auth_token_count,
                )
                    .unwrap_or_else(|e|
                        ::thruput_sdk::exn::decoding_args_throw(&e));

            // Destructure into named args
            let (#arg_names) = __args;

            // Call the method implementation (passing the contract state, if any,
            // and the args)
            #invoke

            // Make sure any storage nodes are fully dropped (and written to environment).
            // Not technically necessary, given that the storage is wrapped in a Node<T>.
        };

        // Return the result to the VM
        ::thruput_sdk::internal::contract_return_encoded(&__gh_res);
    }};

    // Include method info in debugging context (when enabled)
    let wrapped_call = {
        let method_name = ident.to_string();
        let call_info = quote! {
            ::thruput_sdk::internal::HandlingMethod::new(#call_iface, #call_version, #method_name)
        };

        quote! {
            ::thruput_sdk::with_call_tracking!(#call_info, #wrapped_call)
        }
    };

    // Inline the internal implementation, unless requested otherwise
    let inline = if !cfg!(feature = "no_inline_methods") {
        quote! { #[inline(always)] }
    } else {
        quote! {}
    };

    // Contract exports are only emitted when the "entrypoints" feature is
    // enabled.  The build script passes --features entrypoints; deps
    // that don't enable the feature cannot inject entrypoints.
    //
    // RV32 self-registers each method by emitting an `Entrypoint` record into
    // the `thruput_rv32_entrypoints` input section, which the rv32-runtime
    // linker script brackets and `_start`'s dispatcher scans at runtime,
    // instead of exporting a per-method symbol.
    let rv32_entry_static = format_ident!("__GH_RV32_ENTRY_{}", ident);
    Ok(quote! {

        #[doc=#export_docstr]
        #[cfg(feature = "entrypoints")]
        pub extern "C" fn #extern_name() {
            #internal_name()
        }

        #guards

        // rv32 guest dispatch: place this method's Entrypoint record in the
        // dispatch-table input section. Naming is owned by the SDK end-to-end:
        // this macro emits the section, the rv32-runtime linker script
        // (link/groundhog-rv32.ld) KEEPs and brackets it, and
        // rv32-runtime/src/start.rs walks it. `#[used]` keeps the static
        // through codegen/LTO; KEEP() covers --gc-sections.
        #[cfg(all(feature = "entrypoints", target_arch = "riscv32"))]
        #[used]
        #[unsafe(link_section = "thruput_rv32_entrypoints")]
        #[allow(non_upper_case_globals)]
        static #rv32_entry_static: ::thruput_sdk::__rv32::Entrypoint =
            ::thruput_sdk::__rv32::Entrypoint {
                method_id: #method_id_lit,
                wrapper: #extern_name,
            };

        #inline
        #[allow(non_snake_case)]
        pub fn #internal_name() {
            #wrapped_call
        }
    })
}

/// Literal linker symbols are supplied by the interface declaration. The
/// implementing crate supplies its scope, so host tests can link multiple
/// implementations while duplicates inside one contract still fail codegen.
fn generate_entrypoint_guards(
    sig: &syn::Signature,
    iface_name: Option<&str>,
    iface_vers: &str,
    crate_scope: TokenStream,
) -> TokenStream {
    let ident = &sig.ident;
    let entrypoint_sig =
        contract_util::EntrypointSignature::new(iface_name, iface_vers, sig);
    let export_name = entrypoint_export_name(entrypoint_sig.id());
    let link_name = contract_util::entrypoint_link_name(
        iface_name,
        iface_vers,
        &entrypoint_sig.method_sig,
    );
    let id_guard_ident = format_ident!("__GH_RV32_ENTRY_ID_GUARD_{}", ident);
    let link_guard_ident =
        format_ident!("__GH_RV32_ENTRY_NAME_GUARD_{}", ident);
    let id_guard_name = format!("__thruput_entrypoint_id::{export_name}");
    let scoped_link_guard_name =
        quote!(::core::concat!(#link_name, "__crate__", #crate_scope));
    let scoped_id_guard_name =
        quote!(::core::concat!(#id_guard_name, "__crate__", #crate_scope));
    let scoped_link_guard_ident =
        format_ident!("__GH_ENTRY_NAME_GUARD_CRATE_{}", ident);
    let scoped_id_guard_ident =
        format_ident!("__GH_ENTRY_ID_GUARD_CRATE_{}", ident);
    quote! {
        // rv32 duplicate-entrypoint guards: two zero-sized symbols per
        // method (symtab-only — neither reaches the lowered image) so a
        // colliding dispatch ID fails the build instead of producing two
        // dispatch-table records where the runtime's first-match-wins scan
        // silently routes every call to one impl. Symbol names never reach
        // the lowered rv32 image, so readability is free: the readable ID
        // preimage makes the duplicate-symbol error name the interface and
        // method.
        //
        // The readable ID-preimage symbol is declared FIRST so the common
        // collision (same interface/method key) is reported against it —
        // rustc aborts on the first duplicate symbol in declaration order —
        // naming the interface and method:
        //   error: symbol `__thruput_entrypoint::Ledger::0.4::clash`
        //          is already defined
        #[cfg(all(feature = "entrypoints", target_arch = "riscv32"))]
        #[used]
        #[unsafe(export_name = #link_name)]
        #[allow(non_upper_case_globals)]
        static #link_guard_ident: () = ();

        // Belt-and-braces: the 16-hex dispatch ID (constant prefix keeps
        // name-equality identical to ID-equality, and sorts this guard
        // after the readable one so rustc reports the readable symbol for
        // same-key collisions). Two *distinct* preimages whose truncated
        // hashes collide to one dispatch ID (including an adversarially
        // constructed collision) produce different readable symbols — only
        // this guard turns that case into a build error.
        #[cfg(all(feature = "entrypoints", target_arch = "riscv32"))]
        #[used]
        #[unsafe(export_name = #id_guard_name)]
        #[allow(non_upper_case_globals)]
        static #id_guard_ident: () = ();

        // Crate-scoped twins of the two guards above, emitted on EVERY
        // target and without the entrypoints feature gate, so a duplicate
        // definition inside one crate fails any `cargo build`/`cargo test`
        // at codegen — the loop a contract author (or their AI assistant)
        // actually iterates in — instead of surfacing only on the guest
        // build. (`cargo check` skips codegen and still passes; build/test
        // is the loud gate.)
        //
        // The hex-encoded `<crate>__<version>` scope suffix is what makes always-on
        // emission safe: host test binaries routinely link MANY contract
        // crates (e.g. several implementations of one standard interface,
        // or two semver-distinct instances of one contract package in a
        // migration test), and an rv32 artifact links its dependency
        // contract crates. Those cross-crate key collisions are only real
        // when both crates emit entrypoints into one guest image, which
        // the unscoped rv32 guards above still catch. Within one crate
        // instance, a collision is always a bug — and cargo forbids two
        // instances sharing both name and version.
        #[used]
        #[unsafe(export_name = #scoped_link_guard_name)]
        #[allow(non_upper_case_globals)]
        static #scoped_link_guard_ident: () = ();

        // Belt-and-braces, same reasoning as the unscoped ID guard: two
        // distinct preimages whose truncated hashes collide produce
        // different readable symbols, and only this guard catches them.
        #[used]
        #[unsafe(export_name = #scoped_id_guard_name)]
        #[allow(non_upper_case_globals)]
        static #scoped_id_guard_ident: () = ();

    }
}

fn invoking_crate_scope() -> String {
    encoded_crate_scope(
        &std::env::var("CARGO_PKG_NAME")
            .unwrap_or_else(|_| "unknown-crate".to_string()),
        &std::env::var("CARGO_PKG_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string()),
    )
}

/// Re-export in the trait's macro namespace: `use path::Trait as Alias`
/// imports both, so an impl can invoke the helper through its actual trait
/// path without guessing a companion name in the dependency's module.
pub fn generate_trait_guard_macro(
    item: &syn::ItemTrait,
    config: &Config,
) -> TokenStream {
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};

    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = contract.interface_name();
    let iface_vers = contract.defaulted_interface_vers();
    let ident = &item.ident;
    let vis = &item.vis;
    // macro_export puts helpers at the crate root. An invocation counter also
    // distinguishes cloned input spans and proc_macro2's span-less fallback.
    // The digest separates interfaces across proc-macro crate instances.
    // Callers use the trait alias; helper names never enter the dispatch ABI.
    static NEXT_HELPER: AtomicU64 = AtomicU64::new(0);
    let invocation = NEXT_HELPER.fetch_add(1, Ordering::Relaxed);
    let key = format!(
        "{}:{}:{}:{}",
        env!("CARGO_PKG_VERSION"),
        ident,
        iface_name.as_deref().unwrap_or_default(),
        iface_vers
    );
    let hash = Sha256::digest(key.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let helper =
        format_ident!("__thruput_entrypoint_guards_{}_{}", hash, invocation);
    let rules = contract.methods_sorted().into_iter().map(|method| {
        let sig = method.sig();
        let method_ident = &sig.ident;
        let guards = generate_entrypoint_guards(
            sig,
            iface_name.as_deref(),
            &iface_vers,
            quote!($scope),
        );
        quote! {
            (#method_ident, $scope:literal) => {
                const _: () = { #guards };
            };
        }
    });
    quote! {
        #[doc(hidden)]
        #[allow(non_local_definitions)]
        #[macro_export]
        macro_rules! #helper { #(#rules)* }
        #[doc(hidden)]
        #[allow(unused_imports)]
        #vis use #helper as #ident;
    }
}

pub fn generate_free_function_bundle(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let effective_interface_name = config
        .interface_name()
        .unwrap_or_else(|| "_unnamed_interface".to_string());
    let ident = &item.sig.ident;
    // Free functions remain one-off exports: they do not generate a shared
    // client or Entrypoints wrapper. Their hidden exports module therefore
    // includes the function name by default so multiple one-offs can share an
    // interface without colliding, but an explicit `exports = "..."`
    // override still wins when the caller wants to name that module.
    let exports_mod_ident = if config.exports.is_some() {
        config.internal_exports_mod_ident(&effective_interface_name)
    } else {
        free_function_exports_mod_ident(&effective_interface_name, ident)
    };
    let contract = ConfiguredContract::new(config.clone(), item);
    let exports = generate_method(&contract, item)?;

    Ok(quote! {
        #[allow(non_snake_case)]
        mod #exports_mod_ident {
            use super::*;

            #exports
        }
    })
}

pub fn free_function_exports_mod_ident(
    effective_interface_name: &str,
    ident: &syn::Ident,
) -> syn::Ident {
    syn::Ident::new(
        &format!("__thruput_exports_{}_{}", effective_interface_name, ident),
        Span::call_site(),
    )
}

#[cfg(test)]
mod tests {
    use super::free_function_exports_mod_ident;
    use super::*;
    use darling::FromMeta;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    #[test]
    fn trait_guard_names_are_unique_with_identical_fallback_spans() {
        let item: syn::ItemTrait = syn::parse_quote! {
            pub trait Interface { fn ping(); }
        };
        let config = parse_config("");
        assert_ne!(
            generate_trait_guard_macro(&item, &config).to_string(),
            generate_trait_guard_macro(&item, &config).to_string(),
        );
    }

    #[test]
    fn free_function_hidden_exports_include_interface_and_method_name() {
        let config = parse_config(r#"interface = "Iface""#);
        let ident = syn::Ident::new("ping", Span::call_site());

        assert_eq!(
            free_function_exports_mod_ident(
                &config.interface_name().unwrap(),
                &ident,
            )
            .to_string(),
            "__thruput_exports_Iface_ping"
        );
    }

    #[test]
    fn free_function_hidden_exports_honor_explicit_override() {
        let config =
            parse_config(r#"exports = "custom", interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        assert!(generated.contains("mod custom"));
    }

    #[test]
    fn export_wrappers_use_outlined_calldata_fetch() {
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        // The wrapper must own the buffers and reach the shared,
        // non-generic fetch primitive by direct call.
        assert!(
            generated.contains("fetch_calldata_and_auth_tokens"),
            "generated: {generated}"
        );

        // Nothing in the wrapper may take a function's address: every
        // escaped address is a funcref table slot, and the chain caps the
        // table at 64 initial elements. The old `as fn(...)` handler cast
        // cost ~1 slot per entrypoint.
        assert!(!generated.contains("as fn"), "generated: {generated}");
    }

    #[test]
    fn free_function_exports_emit_rv32_registry_not_export_symbols() {
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        // rv32 now emits a #[used] static into the repo-owned
        // thruput_rv32_entrypoints section (no linkme, no distributed_slice).
        assert!(generated.contains("target_arch = \"riscv32\""));
        assert!(generated.contains("thruput_rv32_entrypoints"));
        assert!(generated.contains("link_section"));

        // The old rv32 dispatch symbols and linkme machinery are gone.
        assert!(!generated.contains(".text.gh_"));
        assert!(!generated.contains("distributed_slice"));
        assert!(!generated.contains("THRUPUT_RV32_ENTRYPOINTS"));
    }

    #[test]
    fn shared_reference_params_are_passed_through_ref_arg() {
        let sig: syn::Signature = syn::parse_quote! {
            fn f(a: u32, b: &Ids, c: &mut Other, d: &[u8])
        };

        let args = method_call_args(&sig).to_string();

        assert_eq!(
            args,
            quote! {
                a,
                ::thruput_sdk::call::RefArg::get(&b),
                c,
                ::thruput_sdk::call::RefArg::get(&d)
            }
            .to_string()
        );
    }

    #[test]
    fn entrypoint_wrapper_symbol_carries_id_preimage_on_rv32() {
        // A duplicate entrypoint ID (colliding interface key + same method
        // name) must be a duplicate-symbol build error, instead of two
        // dispatch-table records where the runtime's first-match-wins scan
        // silently routes every call to one impl. rv32 dispatch goes through
        // the record table, so the guard symbol uses the human-readable ID
        // preimage for a legible error message.
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        let expected_link_name = contract_util::entrypoint_link_name(
            Some("Iface"),
            &super::super::contract::interface_vers_default(),
            &contract_util::NormalizedMethodSignature::new("ping"),
        );
        assert!(
            expected_link_name.starts_with("__thruput_entrypoint::Iface::"),
            "{expected_link_name}"
        );
        assert!(generated.contains(&expected_link_name), "{generated}");
        assert!(
            generated.contains("target_arch = \"riscv32\""),
            "{generated}"
        );
        // Belt-and-braces guard: the 16-hex ID is also emitted as a
        // zero-sized rv32 symbol, so cross-preimage hash collisions (which
        // produce distinct readable symbols) still fail the build.
        assert!(
            generated.contains("__GH_RV32_ENTRY_ID_GUARD_"),
            "{generated}"
        );
    }

    #[test]
    fn entrypoint_emits_crate_scoped_guards_on_every_target() {
        // A duplicate definition within one crate must fail ANY cargo
        // build at codegen — host `cargo build`/`cargo test` included, not
        // just the rv32 guest build. That is carried by a crate-scoped
        // guard-symbol pair emitted with no cfg gate at all: same readable
        // link-name (and 16-hex ID) preimages as the rv32 guards, suffixed
        // with `[<crate>]` so unrelated contract crates linked into one
        // host test binary cannot collide.
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        // Expansion happens in this test process, so the scope suffix is
        // this crate's available package identity. name+version, not name alone:
        // one host binary may link two semver-distinct instances of one
        // contract package, and only name+version identifies an instance.
        let crate_scope = format!(
            "{}__{}",
            hex_symbol_component(&std::env::var("CARGO_PKG_NAME").unwrap()),
            hex_symbol_component(&std::env::var("CARGO_PKG_VERSION").unwrap()),
        );
        let link_name = contract_util::entrypoint_link_name(
            Some("Iface"),
            &super::super::contract::interface_vers_default(),
            &contract_util::NormalizedMethodSignature::new("ping"),
        );
        assert!(generated.contains(&link_name), "{generated}");
        assert!(generated.contains(&crate_scope), "{generated}");
        assert!(generated.contains("concat !"), "{generated}");
        assert!(
            generated.contains("__GH_ENTRY_NAME_GUARD_CRATE_"),
            "{generated}"
        );
        assert!(
            generated.contains("__GH_ENTRY_ID_GUARD_CRATE_"),
            "{generated}"
        );

        // The crate-scoped guards are unconditional: exactly the two rv32
        // guards and the rv32 dispatch record are target-gated, nothing
        // else. (Three cfg'd statics, five statics total per entrypoint.)
        let cfg_gates = generated
            .matches("cfg (all (feature = \"entrypoints\" , target_arch = \"riscv32\"))")
            .count();
        assert_eq!(cfg_gates, 3, "{generated}");
    }

    #[test]
    fn crate_scope_is_injective_and_safe_for_linker_version_scripts() {
        let scope = encoded_crate_scope("my-counter", "0.1.0+dev");

        assert_eq!(scope, "6d792d636f756e746572__302e312e302b646576");
        assert!(
            scope
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'_')
        );
        assert_ne!(scope, encoded_crate_scope("my_2d_counter", "0.1.0+dev"));
    }
}
