//! Parse attributes for the contract macro

use darling::FromMeta;
use proc_macro2::Span;

#[derive(Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    client: Option<syn::Ident>,
    pub test_setup: Option<String>,
    pub storage: Option<String>,
    pub interface: Option<String>,
    pub index: Option<u16>,
}

impl Config {
    pub fn client_ident(&self) -> Option<syn::Ident> {
        self.client.clone()
    }

    pub fn exports_mod_ident(&self) -> syn::Ident {
        ident_or(&self.exports, "exports")
    }

    pub fn test_setup_mod_ident(&self) -> syn::Ident {
        ident_or(&self.test_setup, "test_setup")
    }

    pub fn interface_ident(&self) -> Option<syn::Ident> {
        self.interface.as_ref().map(|s| syn::Ident::new(s, Span::call_site()))
    }

    pub fn interface_index(&self) -> Option<u16> {
        self.index
    }

    pub fn storage(&self) -> bool {
        if let Some(setting) = &self.storage {
            setting.as_str() == "true"
        } else {
            true // default if no storage attr present
        }
    }

}

fn ident_or(maybe_name: &Option<String>, default: &str) -> syn::Ident {
    let span = Span::call_site();
    match maybe_name {
        Some(name_string) => {
            syn::Ident::new(name_string.as_ref(), span)
        }
        None => syn::Ident::new(default, span),
    }
}
