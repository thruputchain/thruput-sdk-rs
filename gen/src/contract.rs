//! A common interface for things that can be turned into contracts:
//! - traits
//! - trait impls
//! - bare impls

use crate::config::Config;
use crate::util::cmp_sig_by_name;
use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro2::Span;
use quote::format_ident;
use syn::spanned::Spanned;
use syn::*;

pub trait Method {
    fn sig(&self) -> &syn::Signature;

    fn attrs(&self) -> Vec<Attribute> {
        vec![]
    }

    fn config(&self) -> Config {
        let meta_list: Vec<NestedMeta> = self
            .attrs()
            .into_iter()
            .filter(|a| !is_doc_attr(a))
            .map(|a| NestedMeta::Meta(a.meta))
            .collect();
        Config::from_list(&meta_list)
            .unwrap_or_else(|e| panic!("Couldn't parse attributes: {}", e))
    }
}

fn is_doc_attr(attr: &Attribute) -> bool {
    matches!(&attr.meta, Meta::NameValue(MetaNameValue { path, .. }) if path.is_ident("doc"))
}

impl Method for syn::ItemFn {
    fn sig(&self) -> &syn::Signature {
        &self.sig
    }
    fn attrs(&self) -> Vec<Attribute> {
        self.attrs.clone()
    }
}

impl Method for syn::ImplItemFn {
    fn sig(&self) -> &syn::Signature {
        &self.sig
    }
    fn attrs(&self) -> Vec<Attribute> {
        self.attrs.clone()
    }
}

impl Method for syn::TraitItemFn {
    fn sig(&self) -> &syn::Signature {
        &self.sig
    }
    fn attrs(&self) -> Vec<Attribute> {
        self.attrs.clone()
    }
}

pub trait ContractSrc {
    fn span(&self) -> Span;

    fn trait_path(&self) -> Option<syn::Path>;

    fn interface_name(&self) -> Option<String>;

    fn interface_vers(&self) -> Option<String>;

    fn defaulted_interface_vers(&self) -> String {
        self.interface_vers().unwrap_or_else(interface_vers_default)
    }

    fn client_ident(&self) -> Option<syn::Ident>;

    fn self_ty(&self) -> Option<&syn::Type>;

    fn throws(&self) -> Option<&syn::Ident> {
        None
    }

    fn throws_list(&self) -> Vec<&syn::Ident> {
        vec![]
    }

    fn methods(&self) -> impl Iterator<Item = impl Method>;

    fn methods_sorted(&self) -> impl IntoIterator<Item = impl Method> {
        let mut mm = self.methods().collect::<Vec<_>>();
        mm.sort_by(cmp_method_by_name);
        mm
    }

    fn make_internal_name(&self, bare_ident: &syn::Ident) -> syn::Ident {
        format_ident!("{}_internal", bare_ident)

        /* If we someday need an internal name unique across interfaces:
         *
        format_ident!(
            "{}{}_{}_internal",
            self.interface_name().map(|s| s + "_").unwrap_or(String::new()),
            self.defaulted_interface_vers().replace(".", "_"),
            bare_ident,
        )
        */
    }
}

pub fn interface_vers_default() -> String {
    // Get the version from the crate being compiled via Cargo's env var
    std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
}

use core::cmp::Ordering;
pub fn cmp_method_by_name(a: &impl Method, b: &impl Method) -> Ordering {
    cmp_sig_by_name(a.sig(), b.sig())
}

impl ContractSrc for syn::ItemTrait {
    fn span(&self) -> Span {
        <Self as Spanned>::span(self)
    }

    fn trait_path(&self) -> Option<syn::Path> {
        Some(self.ident.clone().into())
    }

    fn interface_name(&self) -> Option<String> {
        let ident = &self.ident;

        Some(ident.to_string())
    }

    fn interface_vers(&self) -> Option<String> {
        // TODO: some mechanism for specifying a version
        None
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        let trait_ident = &self.ident;
        Some(format_ident!("{}Client", trait_ident))
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        None
    }

    fn methods(&self) -> impl Iterator<Item = impl Method> {
        self.items.iter().filter_map(|item|
            // Extract all trait methods
            if let syn::TraitItem::Fn(trait_fn) = item {
                Some(trait_fn.clone())
            } else {
                None
            })
    }
}

impl ContractSrc for syn::ItemImpl {
    fn span(&self) -> Span {
        <Self as Spanned>::span(self)
    }

    fn trait_path(&self) -> Option<syn::Path> {
        self.trait_.as_ref().map(|(_, path, _)| path.clone())
    }

    fn interface_name(&self) -> Option<String> {
        let ident = self
            .trait_
            .as_ref()
            .map(|(_, path, _)| path_last_segment_ident(path));
        ident.map(|i| i.to_string())
    }

    fn interface_vers(&self) -> Option<String> {
        // TODO: some mechanism for specifying a version
        None
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        // Always return None: when this impl is for a trait, the trait's
        // client_ident() is the authoritative source (and may carry a custom
        // name like `AdderProxy`).  Synthesising a default here would shadow
        // the trait's custom name via ConfiguredContract::client_ident()'s
        // fallback chain.
        None
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        Some(self.self_ty.as_ref())
    }

    fn methods(&self) -> impl Iterator<Item = impl Method> {
        let is_trait_impl = self.trait_.is_some();

        self.items.iter().filter_map(move |item| {
            match item {
                syn::ImplItem::Fn(impl_fn)
                    // Include only public functions unless this is a trait impl
                    if is_visible(&impl_fn.vis) || is_trait_impl =>
                        Some(impl_fn.clone()),
                _ => None
            }
        })
    }
}

/// A `ContractSrc` that prefers config settings over the contained item
pub struct ConfiguredContract<'a, T> {
    config: Config,
    contract: &'a T,
}

impl<'a, T> ConfiguredContract<'a, T> {
    pub fn new(config: Config, contract: &'a T) -> Self {
        Self { config, contract }
    }
}

impl<'a, T> core::ops::Deref for ConfiguredContract<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.contract
    }
}

impl<'a, T> ContractSrc for ConfiguredContract<'a, T>
where
    T: ContractSrc,
{
    fn span(&self) -> Span {
        self.contract.span()
    }

    fn trait_path(&self) -> Option<syn::Path> {
        self.contract.trait_path()
    }

    fn interface_name(&self) -> Option<String> {
        self.config
            .interface_name()
            .or_else(|| self.contract.interface_name())
    }

    fn interface_vers(&self) -> Option<String> {
        self.config
            .version
            .clone()
            .or_else(|| self.contract.interface_vers())
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        self.config
            .client_ident()
            .or_else(|| self.contract.client_ident())
    }

    fn throws(&self) -> Option<&syn::Ident> {
        self.config.throws().or_else(|| self.contract.throws())
    }

    fn throws_list(&self) -> Vec<&syn::Ident> {
        let config_throws = self.config.throws_list();
        if !config_throws.is_empty() {
            config_throws
        } else {
            self.contract.throws_list()
        }
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        self.contract.self_ty()
    }

    fn methods(&self) -> impl Iterator<Item = impl Method> {
        self.contract.methods()
    }
}

fn path_last_segment_ident(path: &syn::Path) -> &syn::Ident {
    let seg = path.segments.last().unwrap();
    &seg.ident
}

fn is_visible(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}
