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
    id_view_tokens(
        krate,
        &major_tokens(version),
        quote! { ::core::module_path!() },
        declared,
    )
}

/// The one place an id's shape is written down. Callers supply the module
/// path as tokens because they do not agree on it: a declaration takes
/// `module_path!()`, while a crate's own identity must not vary by module.
fn id_view_tokens(
    krate: &syn::Path,
    major: &str,
    module_path: TokenStream,
    declared: &str,
) -> TokenStream {
    quote! {
        #krate::__contract_spec::ContractSpecIdView {
            crate_name: ::core::env!("CARGO_PKG_NAME"),
            major: #major,
            module_path: #module_path,
            declared: #declared,
        }
    }
}

/// The declarer id a bare impl or free-function contract records for its
/// interface: the crate itself (see `SpecInterfaceAnchor::declared_by`).
/// `version` is as for [`spec_id_tokens`].
///
/// Both path segments are empty, so the id is exactly (crate, major). One
/// crate may spread an interface over several blocks, so a `module_path!()`
/// here would make two blocks in different modules look like two declarers
/// and get the crate rejected for spreading its own interface. The empty
/// `declared` also keeps this id from ever colliding with a trait's, which
/// always names the trait.
pub fn crate_id_tokens(
    krate: &syn::Path,
    version: Option<&str>,
) -> TokenStream {
    id_view_tokens(krate, &major_tokens(version), quote! { "" }, "")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sdk_path() -> syn::Path {
        syn::parse_quote! { ::sdk }
    }

    /// A crate's own identity must not vary by module.
    ///
    /// One crate may spread a bare-impl interface across several modules, and
    /// every block has to record the same declarer or canonicalization sees
    /// two declarers under one interface key and rejects the crate for
    /// spreading its own interface. `module_path!()` would do exactly that,
    /// so the id carries an empty path instead.
    #[test]
    fn a_crate_id_does_not_depend_on_the_module_it_expands_in() {
        let id = crate_id_tokens(&sdk_path(), Some("0.1.0")).to_string();
        // The field is there; what must not be there is the macro that would
        // make its value vary by expansion site.
        assert!(
            !id.contains("module_path ! ()"),
            "a crate id must not read module_path!(): {id}"
        );
        assert!(
            id.contains(r#"module_path : "" "#),
            "a crate id carries an empty module path: {id}"
        );
    }

    /// The contrast: a declaration's id is module-scoped, which is what tells
    /// two same-named types in different modules apart.
    #[test]
    fn a_declarations_id_does_depend_on_its_module() {
        let id =
            spec_id_tokens(&sdk_path(), "Meta", Some("0.1.0")).to_string();
        assert!(
            id.contains("module_path ! ()"),
            "a declared id must read module_path!(): {id}"
        );
    }
}
