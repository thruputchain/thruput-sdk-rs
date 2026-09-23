//! Generate interfacing materials from a trait or impl that has
//! been annotated with the `contract` macro.  This crate produces:
//!
//! - Client-stub implementations for making contract calls
//! - Guest method stubs and external symbols for calling into the contract
//! - Test-registration code

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;

mod common;
mod contract;
mod contract_exception;
mod contract_type;
mod derive_contract_type_spec;
mod derive_event;
mod derive_exn;
mod static_hash;

/// Early check that all enum variants have explicit discriminants.
/// Called by attribute macros before emitting derives, so the user
/// sees one error instead of cascading failures from multiple derives.
fn check_enum_discriminants(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<()> {
    for variant in variants {
        if variant.discriminant.is_none() {
            return Err(syn::Error::new_spanned(
                variant,
                "enum variants must use explicit discriminants for contract specs",
            ));
        }
    }
    Ok(())
}

/// Shared helper: generate `impl ContractTypeName for #ident`.
/// Used by both `#[contract_type]` and `#[contract_exception]`.
///
/// `name = "..."` replaces the final segment of the id, substituted verbatim:
/// a value carrying `::` spells its own segments, and nothing rejects it. The
/// crate name and major version still prefix every id and come from the
/// declaring crate, so a name cannot reach another crate's id — the most it
/// can do is collide with another type in its own, which is the author's
/// conflict to resolve. The id itself comes from
/// [`common::spec_id::spec_id_tokens`], which every spec-named item shares.
///
/// `version` is the item's declared identity version, or `None` for the
/// crate's. `#[contract_exception(version = "...")]` pins one to hold its
/// selector still across releases, and the id has to be pinned with it.
fn contract_type_name_impl(
    krate: &syn::Path,
    ident: &syn::Ident,
    name: Option<&str>,
    version: Option<&str>,
) -> proc_macro2::TokenStream {
    let declared = name.map_or_else(|| ident.to_string(), str::to_string);
    let id = common::spec_id::spec_id_tokens(krate, &declared, version);
    quote! {
        impl #krate::__contract_spec::ContractTypeName for #ident {
            const NAME: &'static str = #id;
        }
    }
}

mod deletable;
mod storage;

/// Compute a `StorageKey` at compile time: sha256 of the string literal's
/// **value** (quotes stripped, escapes resolved).
///
/// `static_key!("counter")` hashes the 7 bytes `counter`, so the key can be
/// reproduced off-chain (indexer, client, another language) as
/// `sha256("counter")`. Source spelling does not matter:
/// `static_key!(r"a")` == `static_key!("a")`.
#[proc_macro]
pub fn static_key(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);

    static_hash::storage_key_static(input)
}

/// Compute a `SHA256Hash` at compile time: sha256 of the string literal's
/// **value** (quotes stripped, escapes resolved).
///
/// `sha256_static!("foo")` hashes the 3 bytes `foo` and agrees with
/// `sdk::crypto::sha256(b"foo")` at runtime and with
/// [`hash_to_array!`](macro@hash_to_array).
#[proc_macro]
pub fn sha256_static(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);

    static_hash::sha256_static(input)
}

/// Anchor a `#[contract]` trait's spec for inclusion. Spec shaking drops a
/// trait's (candidate) interface spec unless something anchors it — normally a
/// `#[contract] impl`. Use this in an interface crate's spec-carrier `main.rs`
/// so building that crate on its own emits the interface's full spec:
///
/// ```ignore
/// sdk::export_interface_spec!(groundhog_erc20_if::ERC20);
/// ```
#[proc_macro]
pub fn export_interface_spec(item: TokenStream) -> TokenStream {
    let path = syn::parse_macro_input!(item as syn::Path);
    common::spec_reg::generate_interface_anchor_for_trait(&path).into()
}

/// Compute a `SHA256Hash` at compile time: sha256 of the bare identifier's
/// text (no quotes — `sha256_ident!(foo)` hashes the 3 bytes `foo`).
#[proc_macro]
pub fn sha256_ident(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Ident);

    static_hash::sha256_ident(input)
}

/// Like [`sha256_ident!`](macro@sha256_ident) (sha256 of the bare
/// identifier's text), but expands to a `[u8; 32]` array instead of a
/// `SHA256Hash`.
#[proc_macro]
pub fn sha256_array_ident(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Ident);

    static_hash::sha256_array_ident(input)
}

/// Compute a `[u8; 32]` at compile time: sha256 of the literal's **value**
/// (quotes stripped, escapes resolved). Accepts string and byte-string
/// literals: `hash_to_array!("foo")` == sha256 of the 3 bytes `foo` and
/// agrees with `sdk::crypto::sha256(b"foo")` at runtime and with
/// [`sha256_static!`](macro@sha256_static)/[`static_key!`](macro@static_key)
/// for the same string.
#[proc_macro]
pub fn hash_to_array(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::Lit);

    static_hash::sha256_array(input)
}

/// Turn a trait, impl block, or free function into contract entrypoints,
/// generating exports, a typed client, and spec registration.
///
/// # State
///
/// Methods taking `&self`/`&mut self` load the contract's state from
/// storage: the self type must be a storage struct (annotate it with
/// `#[storage]`; a unit struct like `struct Token;` works for contracts
/// that keep no state in the struct). Contracts with only associated
/// functions need no storage annotation.
///
/// # Method visibility (bare impl blocks)
///
/// In a bare (non-trait) impl block, `pub` methods become entrypoints
/// and private methods are skipped (use them for helpers). Restricted
/// visibility (`pub(crate)`, `pub(super)`, `pub(in ...)`) is a compile
/// error because the intent is ambiguous. Trait impls export every
/// trait method regardless of visibility.
///
/// # Arguments
///
/// `version = "x.y.z"`, `interface = "Name"`, `exports = "mod_name"`,
/// `client = ClientIdent`, `throws = Error` / `throws(E1, E2)`,
/// `emits(Event1, Event2)`.
///
/// No `throws(...)` type may contain a `sdk::signer::Signed`: abort frames
/// carry no auth-token handles, so such a payload could only decode to an
/// uninitialized sentinel. Spec canonicalization rejects it when the
/// contract is built. See `#[contract_exception]`.
///
/// # Limitations
///
/// Generic parameters — including explicit lifetime parameters — are not
/// supported on `#[contract]` items or their methods: generated clients,
/// entrypoint wrappers, and spec consts are non-generic, so a spliced
/// `T` or `'a` cannot resolve there. Elided borrow lifetimes (`&self`,
/// `&[u8]`) are fine, and `#[derive(Event)]` types may carry lifetime
/// parameters (folded to `'static` in the spec). Associated types are not
/// supported in `#[contract]` traits.
/// A struct must carry only one `#[contract]` impl block: depending on
/// configuration a second block either collides on generated items
/// (E0428 on names you never wrote) or compiles while silently merging
/// the entrypoint surface — neither is supported.
///
/// # Interface id
///
/// A `#[contract]` trait gains an associated `INTERFACE_ID: sdk::InterfaceId`,
/// derived from the effective interface name and major version (the pair
/// that also namespaces its entrypoint IDs), and the generated
/// `<Iface>Entrypoints` wrapper carries the same constant. The name is
/// reserved on contract traits. Every `#[contract]` impl block also records
/// its interface's id in the contract's interface table
/// (`sdk::interface_table`), which is what `groundhog-erc165-if` answers
/// `supports_interface` from.
///
/// # Storage-domain collision detection
///
/// When a contract uses multiple storage structs, `#[contract]` generates a
/// lib test that verifies their domain strings are all distinct. This check
/// runs as a `#[cfg(test)]` lib test; a crate that sets `[lib] test = false`
/// in its `Cargo.toml`, or that only runs an external test harness without
/// compiling lib tests, will silently skip this collision check at build time.
#[proc_macro_attribute]
pub fn contract(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    // Parse the item tokens into syntax defined in `syn`
    let item = syn::parse_macro_input!(item_tokens as syn::Item);

    // Apply `#[contract]` to its target item
    contract::apply(attr_args, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Storage, attributes(slot, storage_domain))]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    storage::generate_impls(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Mark a struct as contract storage: generates `From<Keypath>` so each
/// field is bound to a storage slot derived from the struct's keypath.
///
/// Works on named-field structs (fields must themselves be storage
/// types like `NNInt`, `Map`, `Value`, or nested storage structs) and
/// on unit structs (`struct Token;`) for contracts that keep all state
/// behind raw storage keys.
///
/// Accepts an optional `domain = "..."` argument to override the default
/// domain (the struct's bare identifier name).
#[proc_macro_attribute]
pub fn storage(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let attr_args: proc_macro2::TokenStream = attr_args.into();
    let item = syn::parse_macro_input!(item_tokens as syn::Item);

    // Accept only `domain = "..."`; reject anything else.
    let mut domain: Option<syn::LitStr> = None;
    if !attr_args.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("domain") {
                domain = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("#[storage] only accepts `domain = \"...\"`"))
            }
        });
        if let Err(e) = syn::parse::Parser::parse2(parser, attr_args) {
            return e.into_compile_error().into();
        }
    }

    let domain_attr = match domain {
        Some(lit) => quote::quote! { #[storage_domain = #lit] },
        None => quote::quote! {},
    };

    quote::quote! {
        #[derive(::sdk::Storage)]
        #domain_attr
        #item
    }
    .into()
}

#[proc_macro_derive(Deletable)]
pub fn derive_deletable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    deletable::generate_impls(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// `#[exn]` or `#[exn(version = "1.0.0")]` — emits `impl Exn` with a
/// computed selector. Use this when you handle your own derives.
/// For the full convenience macro, use `#[contract_exception]` instead.
///
/// Handling your own derives means owning the rule that
/// `#[contract_exception]` enforces structurally: **an exception payload
/// may not contain a `sdk::signer::Signed`**, because abort
/// frames carry no auth-token handles and such a field could only decode to
/// an uninitialized sentinel. See `#[contract_exception]` for the full
/// reasoning. Nothing here can check that for you, but the contract's spec
/// is validated at build time and will reject it.
#[proc_macro_attribute]
pub fn exn(attr_args: TokenStream, item_tokens: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    match derive_exn::parse_args(attr_args) {
        Ok(args) => derive_exn::expand(args, item),
        Err(err) => err,
    }
}

/// `#[contract_exception]` or `#[contract_exception(version = "1.0.0", name = "...")]` —
/// full convenience macro: derives + `#[repr(i32)]` + `impl Exn` +
/// `impl ContractTypeName`.
///
/// # Exception payloads cannot carry auth tokens
///
/// No field may be — or contain — a `sdk::signer::Signed`.
///
/// A `Signed` occupies zero wire bytes; its real handle arrives out of band
/// in the auth-token list the host returns beside a *successful* return.
/// Abort frames carry no such list (the host refuses to return auth tokens
/// on a revert frame), so a `Signed` reached through an exception decodes
/// to the uninitialized `u32::MAX` sentinel — a value that looks like an
/// auth token, is not one, and faults when consumed or returned.
///
/// There is no way to write this correctly, so hand the token back through
/// a successful return instead, or identify it by something inert (the
/// minter's address, an id) and let the caller re-obtain it.
///
/// Two things catch it. This macro's `PartialEq` derive (required by `Exn`)
/// blocks the direct route — `Signed` does not implement `PartialEq` —
/// so reaching the situation at all takes hand-written impls (or `#[exn]`).
/// Beyond that, spec canonicalization walks each `throws(...)` payload when
/// the contract is built and rejects any that reaches a signed token,
/// however deeply nested, so hand-rolled types do not slip through.
#[proc_macro_attribute]
pub fn contract_exception(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    let args = if attr_args.is_empty() {
        contract_exception::ContractExceptionArgs::default()
    } else {
        let meta_list = match darling::ast::NestedMeta::parse_meta_list(
            attr_args.into(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return TokenStream::from(
                    darling::Error::from(e).write_errors(),
                );
            }
        };
        match contract_exception::ContractExceptionArgs::from_list(&meta_list)
        {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
    contract_exception::expand(args, item).into()
}

#[proc_macro_derive(Event, attributes(topic, event_spec))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_event::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Make a struct or enum usable as a contract argument, return value, or
/// auth-token payload: derives XDR codecs, spec registration,
/// auth-handle walkers, and the `Mintable` preimage machinery.
///
/// Both structs and enums recurse when minting auth tokens: a
/// `Signed<T>` field — including one inside an enum variant — becomes a
/// child auth-token node, and its preimage type in the generated
/// `Preimage` mirror is `AuthTokenPreimage<T::Preimage>`.
///
/// Enum variants must carry explicit discriminants.
#[proc_macro_attribute]
pub fn contract_type(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    let args = if attr_args.is_empty() {
        contract_type::ContractTypeArgs::default()
    } else {
        let meta_list = match darling::ast::NestedMeta::parse_meta_list(
            attr_args.into(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return TokenStream::from(
                    darling::Error::from(e).write_errors(),
                );
            }
        };
        match contract_type::ContractTypeArgs::from_list(&meta_list) {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
    contract_type::expand(args, item).into()
}

/// Derive `ContractTypeSpec` (plus `ContractTypeRefSpec`, a linker anchor, and
/// the raw typedef link-section fragment) for a struct or enum.
///
/// # Options
///
/// - `#[contract_type_spec(export)]` — emit the fragment as an *exported*
///   typedef, so it survives spec shaking without an inbound reference.
/// - `#[contract_type_spec(crate = "...")]` — resolve the emitted paths through
///   the given crate path instead of `::sdk`.
///
/// # `crate = "..."`
///
/// The derive normally emits literal `::sdk::…` paths, so a crate that `sdk`
/// itself depends on cannot use it — naming `::sdk` from below `sdk` is a
/// dependency cycle. But everything the derive reaches for is an `sdk`
/// *re-export* of a crate below `sdk`: `__contract_spec` is
/// `groundhog-contract-spec` and `__xdrs` is `xdrs`. Nothing here is owned by
/// `sdk`.
///
/// So such a crate can supply a shim matching those two names and point the
/// derive at it:
///
/// ```ignore
/// #[doc(hidden)]
/// pub mod __sdk_paths {
///     pub use groundhog_contract_spec as __contract_spec;
///     pub use xdrs as __xdrs;
/// }
///
/// #[derive(ContractTypeSpec)]
/// #[contract_type_spec(crate = "crate::__sdk_paths")]
/// pub struct U256([u32; 8]);
/// ```
///
/// Note the `crate::` prefix. The derive is applied *inside* the crate that
/// owns the shim, and a crate cannot name itself as `::its_own_name` without a
/// separate `extern crate self as its_own_name;` — spelling it
/// `::groundhog_u256::__sdk_paths` there fails with E0433. An absolute path is
/// right only when some *other* crate owns the shim.
///
/// The consumer still supplies its own `impl ContractTypeName`, which the
/// derive references but never emits.
///
/// Note this covers *spec* impls only. `#[contract_type]` additionally emits
/// impls of traits genuinely owned by `sdk` (`CollectAuthHandles`,
/// `ReceiveAuthHandles`, `MintableType`), which a lower crate cannot implement
/// — that is a dependency problem, not a path problem, and has no override.
#[proc_macro_derive(ContractTypeSpec, attributes(contract_type_spec))]
pub fn derive_contract_type_spec(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_contract_type_spec::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
