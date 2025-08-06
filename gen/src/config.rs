//! Parse attributes for the contract macro

use darling::FromMeta;
use proc_macro2::Span;

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    pub test_setup: Option<String>,
    pub storage: Option<String>,
    pub interface: Option<String>,
    pub index: Option<u16>,

    client: Option<syn::Ident>,
    err_type: Option<syn::Ident>,
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

    pub fn test_setup_mod_ident(&self) -> syn::Ident {
        ident_or(&self.test_setup, "test_setup")
    }

    pub fn storage(&self) -> bool {
        if let Some(setting) = &self.storage {
            setting.as_str() == "true"
        } else {
            true // default if no storage attr present
        }
    }

    pub fn err_type(&self) -> Option<&syn::Ident> {
        self.err_type.as_ref()
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
