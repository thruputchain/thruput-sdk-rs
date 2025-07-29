//! A common interface for things that can be turned into contracts:
//! - traits
//! - trait impls
//! - bare impls

use syn::spanned::Spanned;
use quote::format_ident;

pub trait ContractSrc: Spanned {
    fn trait_path(&self) -> Option<syn::Path>;
    fn interface_ident(&self) -> Option<&syn::Ident>;
    fn generics(&self) -> &syn::Generics;
    fn methods(&self) -> impl Iterator<Item = &syn::Signature>;
    fn client_ident(&self) -> Option<syn::Ident>;
}

impl ContractSrc for syn::ItemTrait {
    fn trait_path(&self) -> Option<syn::Path> {
        Some(self.ident.clone().into())
    }

    fn interface_ident(&self) -> Option<&syn::Ident> {
        Some(&self.ident)
    }

    fn generics(&self) -> &syn::Generics {
        &self.generics
    }

    fn methods(&self) -> impl Iterator<Item = &syn::Signature> {
        self.items.iter().filter_map(|item| {
            // Extract all trait methods
            if let syn::TraitItem::Fn(trait_fn) = item {
                Some(&trait_fn.sig)
            } else {
                None
            }
        })
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        let trait_ident = &self.ident;
        Some(format_ident!("{}Client", trait_ident))
    }
}

impl ContractSrc for syn::ItemImpl {
    fn trait_path(&self) -> Option<syn::Path> {
        self.trait_.clone().map(|(_, path, _)| path)
    }

    fn interface_ident(&self) -> Option<&syn::Ident> {
        self.trait_.as_ref().map(|(_, path, _)|
            path_last_segment_ident(path))
    }

    fn generics(&self) -> &syn::Generics {
        &self.generics
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

    fn client_ident(&self) -> Option<syn::Ident> {
        None
    }
}

fn path_last_segment_ident(path: &syn::Path) -> &syn::Ident {
    let seg = path.segments.last().unwrap();
    &seg.ident
}

fn is_visible(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}
