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

/// A const expression of type [`ContractSpecIdView<'static>`] holding the spec
/// id of an item declared at the expansion site under the name `declared`.
///
/// Every segment is emitted whole, from something already in hand at the
/// declaration -- nothing here takes a string apart. The module path is only
/// known where the item is declared and a macro cannot see it, so
/// `module_path!()` is emitted for the compiler to expand in place, and
/// `env!("CARGO_PKG_NAME")` likewise resolves in the declaring crate. Note
/// that the two spell the crate differently: `module_path!()` leads with the
/// Rust ident (underscored) while `CARGO_PKG_NAME` is the package name
/// (hyphenated), and the id carries both.
///
/// `version` is the identity version the item declares -- `#[contract_exception
/// (version = "...")]` -- or `None` to take the declaring crate's. It must be
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
    let major = major_tokens(version);
    quote! {
        #krate::__contract_spec::ContractSpecIdView {
            crate_name: ::core::env!("CARGO_PKG_NAME"),
            major: #major,
            module_path: ::core::module_path!(),
            declared: #declared,
        }
    }
}

/// The declarer id a bare impl or free-function contract records for its
/// interface: the crate itself, with no declared name (see
/// `SpecInterfaceAnchor::declared_by`). `version` is as for
/// [`spec_id_tokens`].
///
/// There is no trait to name, and one crate may spread such an interface over
/// several impl blocks or modules, so the crate is the identity. An empty
/// `declared` is what says so, and it is why such an id can never collide with
/// a trait's, which always names the trait.
///
/// Built through [`spec_id_tokens`] rather than assembled here: a declarer is
/// an id like any other, and there is one place that decides what an id looks
/// like.
pub fn crate_id_tokens(
    krate: &syn::Path,
    version: Option<&str>,
) -> TokenStream {
    spec_id_tokens(krate, "", version)
}

/// The major version, by the same rule interface versions use: the leftmost
/// non-zero component, so semver-compatible releases share an id and only a
/// breaking release changes it. `None` takes the declaring crate's version.
fn major_tokens(version: Option<&str>) -> String {
    groundhog_contract_util::semver::major_version_from_string(
        &version.map_or_else(
            crate::common::contract::interface_vers_default,
            str::to_string,
        ),
    )
}
