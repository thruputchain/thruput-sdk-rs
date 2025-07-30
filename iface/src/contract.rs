//! A common interface for things that can be turned into contracts:
//! - traits
//! - trait impls
//! - bare impls

use quote::format_ident;
use super::util::cmp_method_ref_by_name;
use super::config::Config;

pub trait ContractSrc {
    fn trait_path(&self) -> Option<syn::Path>;

    fn interface_name(&self) -> Option<String>;

    fn client_ident(&self) -> Option<syn::Ident>;

    fn self_ty(&self) -> Option<&syn::Type>;

    fn methods(&self) -> impl Iterator<Item = &syn::Signature>;

    fn methods_sorted(&self) -> impl IntoIterator<Item = &syn::Signature> {
        let mut mm = self.methods().collect::<Vec<_>>();
        mm.sort_by(cmp_method_ref_by_name);
        mm
    }
}

impl ContractSrc for syn::ItemTrait {
    fn trait_path(&self) -> Option<syn::Path> {
        Some(self.ident.clone().into())
    }

    fn interface_name(&self) -> Option<String> {
        let ident = &self.ident;

        Some(ident.to_string())
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        let trait_ident = &self.ident;
        Some(format_ident!("{}Client", trait_ident))
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        None
    }

    fn methods(&self) -> impl Iterator<Item = &syn::Signature> {
        self.items.iter().filter_map(|item|
            // Extract all trait methods
            if let syn::TraitItem::Fn(trait_fn) = item {
                Some(&trait_fn.sig)
            } else {
                None
            })
    }
}

impl ContractSrc for syn::ItemImpl {
    fn trait_path(&self) -> Option<syn::Path> {
        self.trait_.as_ref().map(|(_, path, _)| path.clone())
    }

    fn interface_name(&self) -> Option<String> {
        let ident = self.trait_.as_ref().map(|(_, path, _)|
                        path_last_segment_ident(path));
        ident.map(|i| i.to_string())
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        None
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        Some(self.self_ty.as_ref())
    }

    fn methods(&self) -> impl Iterator<Item = &syn::Signature> {
        let is_trait_impl = self.trait_.is_some();

        self.items.iter().filter_map(move |item| {
            match item {
                syn::ImplItem::Fn(impl_fn)
                    // Include only public functions unless this is a trait impl
                    if is_visible(&impl_fn.vis) || is_trait_impl =>
                        Some(&impl_fn.sig),
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
    T: ContractSrc
{
    fn trait_path(&self) -> Option<syn::Path> {
        self.contract.trait_path()
    }

    fn interface_name(&self) -> Option<String> {
        self.config.interface_name().or_else(||
            self.contract.interface_name())
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        self.config.client_ident().or_else(||
            self.contract.client_ident())
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        self.contract.self_ty()
    }

    fn methods(&self) -> impl Iterator<Item = &syn::Signature> {
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
