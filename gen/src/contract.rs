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
use quote::{format_ident, quote};
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
            Ok(lower_contract_type(&ty)?.view)
        })
        .collect()
}

pub fn alloc_requirement_assertions(
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    let mut assertions = Vec::new();

    for method in contract.methods_sorted() {
        for arg in method.sig().inputs.iter() {
            if let syn::FnArg::Typed(pat_type) = arg {
                assertions.push(
                    alloc_requirement_assertion_tokens(pat_type.ty.as_ref())?,
                );
            }
        }

        if let syn::ReturnType::Type(_, ty) = &method.sig().output {
            assertions.push(alloc_requirement_assertion_tokens(ty.as_ref())?);
        }
    }

    for thrown in contract.throws_list() {
        let path = thrown.ty();
        let ty: syn::Type = syn::parse_quote! { #path };
        assertions.push(alloc_requirement_assertion_tokens(&ty)?);
    }

    if contract.trait_path().is_some() && contract.self_ty().is_some() {
        if let Some(client_error_ty) =
            trait_client_error_type(contract.trait_path().as_ref().unwrap())
        {
            assertions.push(
                alloc_requirement_assertion_tokens(&client_error_ty)?,
            );
        }
    }

    Ok(quote! {
        #(#assertions)*
    })
}

pub fn client_error_alloc_spec_tokens(
    client_ident: &syn::Ident,
    throws_list: &[ThrownType],
) -> Result<TokenStream> {
    let error_ident = format_ident!("{}Error", client_ident);
    let requires_alloc = throws_list.iter().map(|thrown| {
        let path = thrown.ty();
        quote! { || ::sdk::__contract_spec::requires_alloc::<#path>() }
    });

    Ok(quote! {
        impl ::sdk::__contract_spec::ContractAllocSpec for #error_ident {
            const REQUIRES_ALLOC: bool = false #(#requires_alloc)*;
        }
    })
}

pub fn alloc_requirement_assertion_tokens(
    ty: &syn::Type,
) -> Result<TokenStream> {
    let ty_string = quote!(#ty).to_string();
    let query_ty = alloc_requirement_query_type(ty);
    let message = format!(
        "`{}` requires `sdk = {{ ..., features = [\"alloc\"] }}` because it uses owned ABI allocation",
        ty_string
    )
    .replace('{', "{{")
    .replace('}', "}}");

    Ok(quote! {
        const _: () = {
            if ::sdk::__contract_spec::requires_alloc::<#query_ty>()
                && !::sdk::internal::SDK_ALLOC_ENABLED
            {
                panic!(#message);
            }
        };
    })
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
            Ok(lower_contract_type(&ty)?.type_defs)
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

pub struct LoweredContractType {
    pub view: TokenStream,
    pub type_defs: TokenStream,
}

pub fn lower_contract_type(ty: &syn::Type) -> Result<LoweredContractType> {
    match ty {
        syn::Type::Reference(reference) => {
            lower_contract_type(&reference.elem)
        }
        syn::Type::Path(_) => Ok(LoweredContractType {
            view: quote! {
                ::sdk::__contract_spec::type_ref::<#ty>()
            },
            type_defs: quote! {
                ::sdk::__contract_spec::type_defs::<#ty>()
            },
        }),
        syn::Type::Paren(paren) => lower_contract_type(&paren.elem),
        syn::Type::Slice(slice) if is_u8_type(slice.elem.as_ref()) => {
            Ok(LoweredContractType {
                view: quote! {
                    ::sdk::__contract_spec::type_ref::<#ty>()
                },
                type_defs: quote! {
                    ::sdk::__contract_spec::ContractTypeDefsView::Empty
                },
            })
        }
        syn::Type::Array(_) | syn::Type::Tuple(_) => Ok(LoweredContractType {
            view: quote! {
                ::sdk::__contract_spec::type_ref::<#ty>()
            },
            type_defs: quote! {
                ::sdk::__contract_spec::type_defs::<#ty>()
            },
        }),
        syn::Type::ImplTrait(_)
        | syn::Type::TraitObject(_)
        | syn::Type::BareFn(_)
        | syn::Type::Ptr(_) => Err(error(
            ty,
            "unsupported contract spec type; supported shapes are named types, tuples, arrays, options, vecs, and references to supported types",
        )),
        _ => Err(error(
            ty,
            "unsupported contract spec type; supported shapes are named types, tuples, arrays, options, vecs, and references to supported types",
        )),
    }
}

pub fn contract_type_view_tokens(
    ty: &syn::Type,
) -> Result<proc_macro2::TokenStream> {
    Ok(lower_contract_type(ty)?.view)
}

pub fn contract_type_typedef_tokens(ty: &syn::Type) -> Result<TokenStream> {
    Ok(lower_contract_type(ty)?.type_defs)
}

fn is_u8_type(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(type_path)
            if path_last_segment_ident(&type_path.path) == "u8"
    )
}

fn alloc_requirement_query_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(reference) => {
            alloc_requirement_query_type(&reference.elem)
        }
        syn::Type::Paren(paren) => alloc_requirement_query_type(&paren.elem),
        _ => ty,
    }
}

fn trait_client_error_type(trait_path: &syn::Path) -> Option<syn::Type> {
    let trait_ident = path_last_segment_ident(trait_path);
    let client_error_ident = format_ident!("{}ClientError", trait_ident);
    let prefix = trait_path_prefix(trait_path);
    let path = if let Some(prefix) = prefix {
        let mut path = prefix;
        path.segments.push(syn::PathSegment {
            ident: client_error_ident,
            arguments: syn::PathArguments::None,
        });
        path
    } else {
        syn::parse_quote! { crate::#client_error_ident }
    };

    Some(syn::Type::Path(syn::TypePath { qself: None, path }))
}

fn trait_path_prefix(path: &syn::Path) -> Option<syn::Path> {
    if path.segments.len() <= 1 {
        return None;
    }

    let mut segments = syn::punctuated::Punctuated::new();
    for segment in path.segments.iter().take(path.segments.len() - 1) {
        segments.push(segment.clone());
    }

    Some(syn::Path {
        leading_colon: path.leading_colon,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use super::{
        alloc_requirement_assertion_tokens, alloc_requirement_assertions,
        client_error_alloc_spec_tokens, contract_type_typedef_tokens,
        contract_type_view_tokens,
        ConfiguredContract,
    };
    use darling::FromMeta;
    use darling::ast::NestedMeta;
    use quote::quote;

    #[test]
    fn alias_paths_use_trait_resolution_for_spec_types() {
        let ty: syn::Type = syn::parse_quote!(Amount);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: Named"));
    }

    #[test]
    fn alias_paths_use_trait_resolution_for_spec_typedefs() {
        let ty: syn::Type = syn::parse_quote!(PayloadList);

        let tokens = contract_type_typedef_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_defs"));
        assert!(!tokens.contains("maybe_type_def"));
    }

    #[test]
    fn fixed_u8_arrays_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!([u8; 32]);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: Named"));
    }

    #[test]
    fn option_of_byte_slice_lowers_structurally() {
        let ty: syn::Type = syn::parse_quote!(Option<&[u8]>);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();
        let typedef_tokens =
            contract_type_typedef_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Option < & [u8] > >"));
        assert!(
            typedef_tokens.contains("type_defs :: < Option < & [u8] > >"),
            "{typedef_tokens}"
        );
    }

    #[test]
    fn option_of_vec_u8_lowers_structurally() {
        let ty: syn::Type = syn::parse_quote!(Option<Vec<u8>>);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();
        let typedef_tokens =
            contract_type_typedef_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Option < Vec < u8 > > >"));
        assert!(!view_tokens.contains("ContractTypeView :: Bytes"));
        assert!(
            typedef_tokens.contains("type_defs :: < Option < Vec < u8 > > >")
        );
    }

    #[test]
    fn tuple_with_vec_u8_lowers_structurally() {
        let ty: syn::Type = syn::parse_quote!((Vec<u8>, bool));

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();
        let typedef_tokens =
            contract_type_typedef_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < (Vec < u8 > , bool) >"));
        assert!(
            typedef_tokens.contains("type_defs :: < (Vec < u8 > , bool) >")
        );
    }

    #[test]
    fn alias_of_u8_collections_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(Blob);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();
        let typedef_tokens =
            contract_type_typedef_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Blob >"));
        assert!(typedef_tokens.contains("type_defs :: < Blob >"));
        assert!(!view_tokens.contains("ContractTypeView :: Bytes"));
    }

    #[test]
    fn alloc_required_aliases_emit_direct_assertions() {
        let ty: syn::Type = syn::parse_quote!(Blob);

        let tokens = alloc_requirement_assertion_tokens(&ty)
            .unwrap()
            .to_string();

        assert!(tokens.contains("requires_alloc :: < Blob > ()"), "{tokens}");
        assert!(tokens.contains("SDK_ALLOC_ENABLED"), "{tokens}");
        assert!(tokens.contains("features = [\\\"alloc\\\"]"), "{tokens}");
        assert!(tokens.contains("Blob"), "{tokens}");
    }

    #[test]
    fn alloc_required_references_are_normalized_before_querying() {
        let ty: syn::Type = syn::parse_quote!(&Blob);

        let tokens = alloc_requirement_assertion_tokens(&ty)
            .unwrap()
            .to_string();

        assert!(tokens.contains("requires_alloc :: < Blob > ()"), "{tokens}");
        assert!(!tokens.contains("requires_alloc :: < & Blob > ()"), "{tokens}");
        assert!(tokens.contains("Blob"), "{tokens}");
    }

    #[test]
    fn free_function_throws_are_included_in_alloc_assertions() {
        let item: syn::ItemFn = syn::parse_quote! {
            fn relay(blob: Blob) -> Blob {
                blob
            }
        };
        let meta_list = NestedMeta::parse_meta_list(
            quote! { throws(BlobError) }.into(),
        )
        .unwrap();
        let config = Config::from_list(&meta_list).unwrap();
        let contract = ConfiguredContract::new(config, &item);

        let tokens = alloc_requirement_assertions(&contract)
            .unwrap()
            .to_string();

        assert!(tokens.contains("requires_alloc :: < Blob > ()"), "{tokens}");
        assert!(
            tokens.contains("requires_alloc :: < BlobError > ()"),
            "{tokens}"
        );
        assert!(tokens.contains("SDK_ALLOC_ENABLED"), "{tokens}");
    }

    #[test]
    fn trait_impl_alloc_assertions_cover_client_error_proxy() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl imported_error_callee::ImportedErrorCallee for Relay {
                fn throw_imported_error() {}
            }
        };
        let contract = ConfiguredContract::new(
            Config::from_list(&[]).unwrap(),
            &item,
        );

        let tokens = alloc_requirement_assertions(&contract)
            .unwrap()
            .to_string();

        assert!(
            tokens.contains(
                "requires_alloc :: < imported_error_callee :: ImportedErrorCalleeClientError > ()"
            ),
            "{tokens}"
        );
        assert!(tokens.contains("SDK_ALLOC_ENABLED"), "{tokens}");
    }

    #[test]
    fn client_error_alloc_spec_tokens_cover_all_declared_throws() {
        let meta_list = NestedMeta::parse_meta_list(
            quote! { throws(BlobError, StringError) }.into(),
        )
        .unwrap();
        let config = Config::from_list(&meta_list).unwrap();
        let client_ident = syn::parse_quote!(ImportedErrorCalleeClient);

        let tokens = client_error_alloc_spec_tokens(
            &client_ident,
            &config.throws_list(),
        )
            .unwrap()
            .to_string();

        assert!(
            tokens.contains("impl :: sdk :: __contract_spec :: ContractAllocSpec for ImportedErrorCalleeClientError"),
            "{tokens}"
        );
        assert!(
            tokens.contains("requires_alloc :: < BlobError > ()"),
            "{tokens}"
        );
        assert!(
            tokens.contains("requires_alloc :: < StringError > ()"),
            "{tokens}"
        );
    }

    #[test]
    fn builtin_byte_blob_types_lower_to_bytes() {
        for ty in [
            syn::parse_quote!(sdk::Ed25519Signature),
            syn::parse_quote!(sdk::Ed25519PublicKey),
            syn::parse_quote!(sdk::ContractEntrypointID),
        ] {
            let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

            assert!(tokens.contains("type_ref"));
            assert!(!tokens.contains("ContractTypeView :: Bytes"));
        }
    }

    #[test]
    fn block_height_lowers_to_i64() {
        let ty: syn::Type = syn::parse_quote!(sdk::BlockHeight);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: I64"));
    }

    #[test]
    fn contract_address_lowers_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(sdk::ContractAddress);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: Address"));
    }

    #[test]
    fn unsupported_type_shapes_are_rejected() {
        for ty in [
            syn::parse_quote!(impl core::fmt::Debug),
            syn::parse_quote!(&dyn core::fmt::Debug),
            syn::parse_quote!(fn(u32) -> u32),
            syn::parse_quote!(*const u8),
        ] {
            let err = contract_type_view_tokens(&ty).unwrap_err();
            assert!(
                err.to_string().contains("unsupported contract spec type"),
                "{}",
                err
            );
        }
    }
}
