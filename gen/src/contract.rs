//! A common interface for things that can be turned into contracts:
//! - traits
//! - trait impls
//! - bare impls

use crate::client::reserved_client_name_error;
use crate::config::{Config, ThrownType};
use crate::util::{cmp_sig_by_name, error};
use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
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

    fn throws(&self) -> Option<ThrownType> {
        None
    }

    fn throws_list(&self) -> Vec<ThrownType> {
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

pub fn validate_reserved_client_method_names<M>(
    methods: impl IntoIterator<Item = M>,
) -> Result<()>
where
    M: Method,
{
    let mut combined: Option<syn::Error> = None;

    for method in methods {
        let sig = method.sig();
        let name = sig.ident.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name);

        if let Some(message) = reserved_client_name_error(name) {
            let err = error(sig, message);
            match combined.as_mut() {
                Some(existing) => existing.combine(err),
                None => combined = Some(err),
            }
        }
    }

    match combined {
        Some(err) => Err(err),
        None => Ok(()),
    }
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

impl ContractSrc for syn::ItemFn {
    fn span(&self) -> Span {
        <Self as Spanned>::span(self)
    }

    fn trait_path(&self) -> Option<syn::Path> {
        None
    }

    fn interface_name(&self) -> Option<String> {
        None
    }

    fn interface_vers(&self) -> Option<String> {
        None
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        None
    }

    fn self_ty(&self) -> Option<&syn::Type> {
        None
    }

    fn methods(&self) -> impl Iterator<Item = impl Method> {
        core::iter::once(self.clone())
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
        if let Some((_, path, _)) = &self.trait_ {
            return Some(path_last_segment_ident(path).to_string());
        }

        if let syn::Type::Path(tp) = &*self.self_ty {
            return tp.path.segments.last().map(|seg| seg.ident.to_string());
        }

        None
    }

    fn interface_vers(&self) -> Option<String> {
        // TODO: some mechanism for specifying a version
        None
    }

    fn client_ident(&self) -> Option<syn::Ident> {
        if self.trait_.is_some() {
            // When implementing a trait, return None so the trait's
            // client_ident() is authoritative (may carry a custom name
            // like `AdderProxy`).
            return None;
        }
        // Bare impl: derive from Self type, e.g. Foo -> FooClient
        if let syn::Type::Path(tp) = &*self.self_ty {
            tp.path
                .segments
                .last()
                .map(|seg| format_ident!("{}Client", seg.ident))
        } else {
            None
        }
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
            .or_else(|| {
                self.config
                    .interface_name()
                    .map(|name| format_ident!("{}Client", name))
            })
            .or_else(|| self.contract.client_ident())
    }

    fn throws(&self) -> Option<ThrownType> {
        self.config.throws().or_else(|| self.contract.throws())
    }

    fn throws_list(&self) -> Vec<ThrownType> {
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

pub fn effective_interface_name(contract: &impl ContractSrc) -> String {
    contract
        .interface_name()
        .unwrap_or_else(|| "_unnamed_interface".to_string())
}

pub fn throws_spec_tokens(
    contract: &impl ContractSrc,
) -> Result<Vec<TokenStream>> {
    contract
        .throws_list()
        .into_iter()
        .map(|thrown| {
            let path = thrown.ty();
            let ty: syn::Type = syn::parse_quote! { #path };
            contract_type_view_tokens(&ty)
        })
        .collect()
}

pub fn throws_typedef_tokens(
    contract: &impl ContractSrc,
) -> Result<Vec<TokenStream>> {
    contract
        .throws_list()
        .into_iter()
        .map(|thrown| {
            let path = thrown.ty();
            let ty: syn::Type = syn::parse_quote! { #path };
            Ok(quote! {
                ::sdk::__contract_spec::maybe_type_def::<#ty>()
            })
        })
        .collect()
}

fn is_visible(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

pub fn method_arg_spec_tokens(
    method: &impl Method,
) -> Result<Vec<proc_macro2::TokenStream>> {
    method
        .sig()
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => Some(pat_type),
            syn::FnArg::Receiver(_) => None,
        })
        .map(|pat_type| {
            let name = match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => pat_ident.ident.to_string(),
                _ => {
                    return Err(error(
                        pat_type,
                        "couldn't interpret method arg",
                    ));
                }
            };
            let ty = contract_type_view_tokens(pat_type.ty.as_ref())?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractFieldView {
                    name: #name,
                    ty: #ty,
                }
            })
        })
        .collect()
}

pub fn method_return_spec_token(
    method: &impl Method,
) -> Result<proc_macro2::TokenStream> {
    match &method.sig().output {
        syn::ReturnType::Default => {
            Ok(quote! { ::sdk::__contract_spec::ContractTypeView::Void })
        }
        syn::ReturnType::Type(_, ty) => contract_type_view_tokens(ty),
    }
}

pub fn contract_type_view_tokens(
    ty: &syn::Type,
) -> Result<proc_macro2::TokenStream> {
    match ty {
        syn::Type::Reference(reference) => match reference.elem.as_ref() {
            syn::Type::Slice(slice) if is_u8_type(slice.elem.as_ref()) => {
                Ok(quote! { ::sdk::__contract_spec::ContractTypeView::Bytes })
            }
            inner => contract_type_view_tokens(inner),
        },
        syn::Type::Slice(slice) if is_u8_type(slice.elem.as_ref()) => {
            Ok(quote! { ::sdk::__contract_spec::ContractTypeView::Bytes })
        }
        syn::Type::Array(array) if is_u8_type(array.elem.as_ref()) => {
            Ok(quote! { ::sdk::__contract_spec::ContractTypeView::Bytes })
        }
        syn::Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                return Ok(
                    quote! { ::sdk::__contract_spec::ContractTypeView::Void },
                );
            }
            let items = tuple
                .elems
                .iter()
                .map(contract_type_view_tokens)
                .collect::<Result<Vec<_>>>()?;
            Ok(quote! {{
                const ITEMS: &[::sdk::__contract_spec::ContractTypeView<'static>] = &[#(#items),*];
                ::sdk::__contract_spec::ContractTypeView::Tuple(ITEMS)
            }})
        }
        syn::Type::Path(type_path) => {
            if path_last_segment_ident(&type_path.path) == "Option" {
                let inner =
                    first_generic_type_arg(type_path).ok_or_else(|| {
                        error(type_path, "Option requires one type argument")
                    })?;
                let inner = contract_type_view_tokens(inner)?;
                return Ok(quote! {{
                    const INNER: ::sdk::__contract_spec::ContractTypeView<'static> = #inner;
                    ::sdk::__contract_spec::ContractTypeView::Option(&INNER)
                }});
            }
            if path_last_segment_ident(&type_path.path) == "Vec" {
                let inner =
                    first_generic_type_arg(type_path).ok_or_else(|| {
                        error(type_path, "Vec requires one type argument")
                    })?;
                if is_u8_type(inner) {
                    return Ok(quote! {
                        ::sdk::__contract_spec::ContractTypeView::Bytes
                    });
                }
                let inner = contract_type_view_tokens(inner)?;
                return Ok(quote! {{
                    const INNER: ::sdk::__contract_spec::ContractTypeView<'static> = #inner;
                    ::sdk::__contract_spec::ContractTypeView::Vec(&INNER)
                }});
            }
            if path_last_segment_ident(&type_path.path) == "ContractAddress" {
                return Ok(quote! {
                    ::sdk::__contract_spec::ContractTypeView::Address
                });
            }
            if path_last_segment_ident(&type_path.path) == "BlockHeight" {
                return Ok(quote! {
                    ::sdk::__contract_spec::ContractTypeView::I64
                });
            }
            if path_last_segment_ident(&type_path.path) == "SHA256Hash"
                || path_last_segment_ident(&type_path.path)
                    == "Ed25519Signature"
                || path_last_segment_ident(&type_path.path)
                    == "Ed25519PublicKey"
                || path_last_segment_ident(&type_path.path)
                    == "ContractEntrypointID"
            {
                return Ok(quote! {
                    ::sdk::__contract_spec::ContractTypeView::Bytes
                });
            }
            Ok(quote! {
                ::sdk::__contract_spec::type_ref::<#ty>()
            })
        }
        syn::Type::Paren(paren) => contract_type_view_tokens(&paren.elem),
        _ => {
            let symbol = canonical_named_type_symbol(ty);
            Ok(quote! {
                ::sdk::__contract_spec::ContractTypeView::Named(#symbol)
            })
        }
    }
}

fn first_generic_type_arg(type_path: &syn::TypePath) -> Option<&syn::Type> {
    let segment = type_path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
    else {
        return None;
    };
    arguments.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn is_u8_type(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(type_path)
            if path_last_segment_ident(&type_path.path) == "u8"
    )
}

fn canonical_named_type_symbol(ty: &syn::Type) -> String {
    ty.to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::contract_type_view_tokens;

    #[test]
    fn alias_paths_use_trait_resolution_for_spec_types() {
        let ty: syn::Type = syn::parse_quote!(Amount);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: Named"));
    }

    #[test]
    fn fixed_u8_arrays_lower_to_bytes() {
        let ty: syn::Type = syn::parse_quote!([u8; 32]);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("ContractTypeView :: Bytes"));
        assert!(!tokens.contains("ContractTypeView :: Named"));
    }

    #[test]
    fn builtin_byte_blob_types_lower_to_bytes() {
        for ty in [
            syn::parse_quote!(sdk::Ed25519Signature),
            syn::parse_quote!(sdk::Ed25519PublicKey),
            syn::parse_quote!(sdk::ContractEntrypointID),
        ] {
            let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

            assert!(tokens.contains("ContractTypeView :: Bytes"));
            assert!(!tokens.contains("type_ref"));
        }
    }

    #[test]
    fn block_height_lowers_to_i64() {
        let ty: syn::Type = syn::parse_quote!(sdk::BlockHeight);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("ContractTypeView :: I64"));
        assert!(!tokens.contains("type_ref"));
    }
}
