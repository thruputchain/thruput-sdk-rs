//! The one place a spec id is derived.
//!
//! Every user-defined thing the spec names — a `#[contract_type]`, a
//! `#[contract_exception]`, a `#[derive(Event)]` — is named by the same id:
//! `CRATE@MAJOR::module::path::Name`. A bare ident cannot tell two of them
//! apart, since two crates, two modules of one crate, or two major versions of
//! one crate may each declare a `Transfer`, and a spec naming both simply
//! cannot say which is which.
//!
//! References resolve through the declaring item rather than by repeating a
//! string: a type reference reads `<T as ContractTypeName>::NAME`, and an
//! interface's `emits(...)` entry reads `<E as ContractEventSpec>::EVENT_DEF
//! .name`. So a reference follows whatever the thing it names calls itself,
//! and this module is the only thing that decides what that is.

use proc_macro2::TokenStream;
use quote::quote;

/// A const expression of type `&'static str` holding the spec id of an item
/// declared at the expansion site under the name `declared`.
///
/// The module path is only known where the item is declared and a macro cannot
/// see it, so `module_path!()` is emitted for the compiler to expand in place.
/// `version` is the identity version the item declares — `#[contract_exception
/// (version = "...")]` — or `None` to take the declaring crate's. It must be
/// whatever that item's other identity derivations use, or the same type ends
/// up with a selector pinned to one version and an id tracking another.
///
/// Where `None`, the version is read here: a proc macro reads the environment
/// of the rustc process compiling the crate the macro is *invoked* in, and
/// these macros are invoked on the item's own declaration, so that is the
/// declaring crate. (The same is not true of `#[contract]` on a trait impl,
/// which is why interface versions need an explicit override there.)
///
/// Emit this at a declaration only. Anywhere else `module_path!()` names the
/// wrong module and the id is a lie.
pub fn spec_id_tokens(
    krate: &syn::Path,
    declared: &str,
    version: Option<&str>,
) -> TokenStream {
    // The major version, by the same rule interface versions use: the leftmost
    // non-zero component, so semver-compatible releases share an id and only a
    // breaking release changes it.
    let major = groundhog_contract_util::semver::major_version_from_string(
        &version.map_or_else(
            crate::common::contract::interface_vers_default,
            str::to_string,
        ),
    );
    quote! {
        {
            const MAJOR: &str = #major;
            const DECLARED: &str = #declared;
            const LEN: usize = #krate::__contract_spec::qualified_id_len(
                ::core::module_path!(),
                MAJOR,
                DECLARED,
            );
            const BYTES: [u8; LEN] =
                #krate::__contract_spec::qualified_id_bytes::<LEN>(
                    ::core::module_path!(),
                    MAJOR,
                    DECLARED,
                );
            match ::core::str::from_utf8(&BYTES) {
                ::core::result::Result::Ok(id) => id,
                // Every input is UTF-8 and the id is spliced on ASCII
                // boundaries, so this is unreachable.
                ::core::result::Result::Err(_) => {
                    ::core::panic!("spec id is not valid UTF-8")
                }
            }
        }
    }
}
