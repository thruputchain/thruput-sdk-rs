//! Parse attributes for the contract macro

use darling::FromMeta;
use darling::util::Flag;
use heck::ToSnakeCase;
use proc_macro2::Span;

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    pub test_setup: Option<String>,
    pub storage: Option<String>,
    pub interface: Option<String>,
    pub version: Option<String>,
    pub index: Option<u16>,
    pub init: Flag,
    pub doc: Option<String>,

    client: Option<syn::Ident>,
    throws: Option<syn::Ident>,
}

impl Config {
    pub fn client_ident(&self) -> Option<syn::Ident> {
        self.client.clone()
    }

    pub fn interface_name(&self) -> Option<String> {
        self.interface.clone()
    }

    pub fn interface_index(&self) -> Option<u16> {
        self.index
    }

    pub fn exports_mod_ident(&self) -> syn::Ident {
        ident_or(&self.exports, "exports")
    }

    /// Returns the exports module identifier.
    /// When `trait_name` is provided and no explicit `exports` was set,
    /// derives the name from the trait (e.g., `Adder` → `adder_exports`).
    pub fn exports_mod_ident_for_trait(&self, trait_name: Option<&str>) -> syn::Ident {
        match (&self.exports, trait_name) {
            (Some(name), _) => syn::Ident::new(name, Span::call_site()),
            (None, Some(name)) => {
                let snake = name.to_snake_case();
                syn::Ident::new(&format!("{snake}_exports"), Span::call_site())
            }
            (None, None) => syn::Ident::new("exports", Span::call_site()),
        }
    }

    pub fn storage(&self) -> bool {
        if let Some(setting) = &self.storage {
            setting.as_str() == "true"
        } else {
            true // default if no storage attr present
        }
    }

    pub fn throws(&self) -> Option<&syn::Ident> {
        self.throws.as_ref()
    }
}

fn ident_or(maybe_name: &Option<String>, default: &str) -> syn::Ident {
    let span = Span::call_site();
    match maybe_name {
        Some(name_string) => syn::Ident::new(name_string.as_ref(), span),
        None => syn::Ident::new(default, span),
    }
}
