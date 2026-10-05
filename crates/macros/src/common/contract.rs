//! A common interface for things that can be turned into contracts:
//! - traits
//! - trait impls
//! - bare impls

use super::client::reserved_client_name_error;
use super::config::{Config, ThrownType};
use super::util::{cmp_sig_by_name, error};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::*;

pub trait Method {
    fn sig(&self) -> &syn::Signature;
    fn attrs(&self) -> &[syn::Attribute];
}

impl Method for syn::ItemFn {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.attrs
    }
    fn sig(&self) -> &syn::Signature {
        &self.sig
    }
}

impl Method for syn::ImplItemFn {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.attrs
    }
    fn sig(&self) -> &syn::Signature {
        &self.sig
    }
}

impl Method for syn::TraitItemFn {
    fn attrs(&self) -> &[syn::Attribute] {
        &self.attrs
    }
    fn sig(&self) -> &syn::Signature {
        &self.sig
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

/// Contract method arguments must be plain identifiers: the generated
/// client and entrypoint code needs a name for every parameter. Reject
/// `_`, tuples, and other patterns with a spanned error instead of
/// letting `method_args` panic mid-expansion.
pub fn validate_method_arg_patterns<M>(
    methods: impl IntoIterator<Item = M>,
) -> Result<()>
where
    M: Method,
{
    let mut combined: Option<syn::Error> = None;

    for method in methods {
        for arg in &method.sig().inputs {
            let syn::FnArg::Typed(pat_type) = arg else {
                continue;
            };
            if !matches!(pat_type.pat.as_ref(), syn::Pat::Ident(_)) {
                let err = error(
                    pat_type,
                    "contract method arguments must be plain identifiers; \
                     patterns like `_`, tuples, or destructuring are not \
                     supported",
                );
                match combined.as_mut() {
                    Some(existing) => existing.combine(err),
                    None => combined = Some(err),
                }
            }
            // A `&mut` argument would be a mutable borrow of a value
            // decoded from calldata that nothing reads back, so it has no
            // meaning a caller could rely on.
            if let syn::Type::Reference(reference) = pat_type.ty.as_ref()
                && let Some(mutability) = reference.mutability
            {
                let err = error(
                    &mutability,
                    "contract method arguments cannot be `&mut`; take the \
                     argument by value, or by shared reference `&`",
                );
                match combined.as_mut() {
                    Some(existing) => existing.combine(err),
                    None => combined = Some(err),
                }
            }
        }
    }

    match combined {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Reject generic parameters on contract methods: the method signatures
/// are spliced into non-generic module-level consts and client fns (spec
/// defs, entrypoint wrappers, clients), so a method-scoped `T` — or an
/// explicit `<'a>` lifetime parameter, verified to die with E0261 inside
/// the generated client — would surface as an error in code the author
/// never wrote. Elided borrow lifetimes (`&self`, `&[u8]`) are not
/// generic parameters and remain fine.
pub fn validate_method_generics<M>(
    methods: impl IntoIterator<Item = M>,
) -> Result<()>
where
    M: Method,
{
    for method in methods {
        super::util::validate_no_generics(
            &method.sig().generics,
            false,
            "contract methods",
        )?;
    }
    Ok(())
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
        self.trait_.as_ref().map(|(path, _)| path.clone())
    }

    fn interface_name(&self) -> Option<String> {
        if let Some((path, _)) = &self.trait_ {
            return Some(path_last_segment_ident(path).to_string());
        }

        if let syn::Type::Path(tp) = &*self.self_ty {
            return tp.path.segments.last().map(|seg| seg.ident.to_string());
        }

        None
    }

    fn interface_vers(&self) -> Option<String> {
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
            .as_ref()
            .map(|v| v.as_str().to_string())
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
            let type_view = contract_type_view_tokens(&ty)?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractExceptionTypeView {
                    selector: <#ty as ::sdk::exn::Exn>::SELECTOR,
                    ty: #type_view,
                }
            })
        })
        .collect()
}

pub fn throws_anchor_tokens(
    contract: &impl ContractSrc,
) -> Result<Vec<TokenStream>> {
    contract
        .throws_list()
        .into_iter()
        .map(|thrown| {
            let path = thrown.ty();
            let ty: syn::Type = syn::parse_quote! { #path };
            contract_type_anchor_token(&ty)
        })
        .collect()
}

fn is_visible(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// Bare-impl contract methods must be either `pub` (exported as an
/// entrypoint) or fully private (an internal helper, skipped). Restricted
/// visibility is ambiguous — the author plausibly expected export — so
/// reject it. Trait impls export every method regardless of visibility.
pub fn validate_bare_impl_method_visibility(
    item: &syn::ItemImpl,
) -> Result<()> {
    if item.trait_.is_some() {
        return Ok(());
    }
    let mut combined: Option<syn::Error> = None;
    for impl_item in &item.items {
        if let syn::ImplItem::Fn(f) = impl_item
            && matches!(f.vis, syn::Visibility::Restricted(_))
        {
            let err = syn::Error::new_spanned(
                &f.vis,
                "ambiguous visibility for contract method: use `pub` \
                 to export it as an entrypoint, or remove the visibility \
                 qualifier to keep it internal",
            );
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

/// Build a `ContractSpecMethodView` literal (name, args, return) for a method.
///
/// A `#[contract]` trait emits these into a `__GROUNDHOG_IFACE_METHODS` const
/// so that a trait impl pulls the *trait's* method signatures into its spec
/// (mirroring how events flow through `__GROUNDHOG_IFACE_EVENTS`). This keeps
/// the trait authoritative: an impl that renames a parameter (e.g. `_factory`)
/// cannot diverge the canonical interface spec.
pub fn method_spec_view_tokens(
    method: &impl Method,
) -> Result<proc_macro2::TokenStream> {
    let name = method.sig().ident.to_string();
    let args = method_arg_spec_tokens(method)?;
    let return_type = method_return_spec_token(method)?;
    let effect = method_effect_tokens(method)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractSpecMethodView {
            name: #name,
            args: &[#(#args),*],
            return_type: #return_type,
            effect: #effect,
        }
    })
}

/// Parse the method's explicit declaration, never its name or receiver.
pub fn method_effect_tokens(method: &impl Method) -> Result<TokenStream> {
    let mut effect = None;
    for attr in method
        .attrs()
        .iter()
        .filter(|attr| attr.path().is_ident("contract"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("effect") {
                return Err(meta.error("expected `effect = \"pure\"`, `\"readOnly\"`, or `\"stateChanging\"`"));
            }
            if effect.is_some() {
                return Err(meta.error("duplicate method effect declaration"));
            }
            let value: syn::LitStr = meta.value()?.parse()?;
            let name = match value.value().as_str() {
                "pure" => "Pure",
                "readOnly" => "ReadOnly",
                "stateChanging" => "StateChanging",
                _ => return Err(error(&value, "expected `pure`, `readOnly`, or `stateChanging`; omit the declaration for unknown effects")),
            };
            effect = Some(syn::Ident::new(name, value.span()));
            Ok(())
        })?;
    }
    let effect = effect
        .unwrap_or_else(|| syn::Ident::new("Unknown", Span::call_site()));
    Ok(quote! { ::sdk::__contract_spec::ContractMethodEffectView::#effect })
}

pub fn method_anchor_tokens(
    method: &impl Method,
) -> Result<Vec<proc_macro2::TokenStream>> {
    let mut tokens = Vec::new();

    for arg in method.sig().inputs.iter() {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                tokens.push(contract_type_anchor_token(pat_type.ty.as_ref())?);
            }
            syn::FnArg::Receiver(_) => {}
        }
    }

    match &method.sig().output {
        syn::ReturnType::Default => tokens.push(quote! {
            <() as ::sdk::__contract_spec::ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT
        }),
        syn::ReturnType::Type(_, ty) => {
            tokens.push(contract_type_anchor_token(ty)?);
        }
    }

    Ok(tokens)
}

/// The crate path spec-emitting macros use unless told otherwise.
///
/// Everything reached through it (`__contract_spec`, `__xdrs`) is an `sdk`
/// re-export of a crate *below* `sdk`, so a crate that `sdk` depends on can
/// name those targets directly and pass its own shim path instead. See
/// `#[contract_type_spec(crate = "...")]`.
pub fn sdk_crate_path() -> syn::Path {
    syn::parse_quote!(::sdk)
}

fn lower_via_type_ref_trait(krate: &syn::Path, ty: &syn::Type) -> TokenStream {
    quote! {
        #krate::__contract_spec::type_ref::<#ty>()
    }
}

pub fn contract_type_anchor_token(ty: &syn::Type) -> Result<TokenStream> {
    match ty {
        syn::Type::Reference(reference) => {
            contract_type_anchor_token(&reference.elem)
        }
        syn::Type::Path(_)
        | syn::Type::Paren(_)
        | syn::Type::Slice(_)
        | syn::Type::Array(_)
        | syn::Type::Tuple(_) => Ok(quote! {
            <#ty as ::sdk::__contract_spec::ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT
        }),
        syn::Type::ImplTrait(_)
        | syn::Type::TraitObject(_)
        | syn::Type::FnPtr(_)
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

pub fn contract_type_view_tokens(ty: &syn::Type) -> Result<TokenStream> {
    contract_type_view_tokens_in(&sdk_crate_path(), ty)
}

/// As [`contract_type_view_tokens`], but resolving the spec traits through
/// `krate` rather than `::sdk`.
pub fn contract_type_view_tokens_in(
    krate: &syn::Path,
    ty: &syn::Type,
) -> Result<TokenStream> {
    match ty {
        syn::Type::Reference(reference) => {
            contract_type_view_tokens_in(krate, &reference.elem)
        }
        syn::Type::Path(_) => Ok(lower_via_type_ref_trait(krate, ty)),
        syn::Type::Paren(paren) => {
            contract_type_view_tokens_in(krate, &paren.elem)
        }
        syn::Type::Slice(_) | syn::Type::Array(_) | syn::Type::Tuple(_) => {
            Ok(lower_via_type_ref_trait(krate, ty))
        }
        syn::Type::ImplTrait(_)
        | syn::Type::TraitObject(_)
        | syn::Type::FnPtr(_)
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
    fn fixed_u8_arrays_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!([u8; 32]);

        let tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(tokens.contains("type_ref"));
        assert!(!tokens.contains("ContractTypeView :: Named"));
    }

    #[test]
    fn byte_slice_references_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(&[u8]);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < [u8] >"));
    }

    #[test]
    fn borrowed_named_types_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(&Payload);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Payload >"));
    }

    #[test]
    fn option_of_byte_slice_lowers_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(Option<&[u8]>);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Option < & [u8] > >"));
    }

    #[test]
    fn option_of_vec_u8_lowers_structurally() {
        let ty: syn::Type = syn::parse_quote!(Option<Vec<u8>>);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Option < Vec < u8 > > >"));
        assert!(!view_tokens.contains("ContractTypeView :: Bytes"));
    }

    #[test]
    fn tuple_with_vec_u8_lowers_structurally() {
        let ty: syn::Type = syn::parse_quote!((Vec<u8>, bool));

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < (Vec < u8 > , bool) >"));
    }

    #[test]
    fn alias_of_u8_collections_lower_via_trait_resolution() {
        let ty: syn::Type = syn::parse_quote!(Blob);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(view_tokens.contains("type_ref :: < Blob >"));
        assert!(!view_tokens.contains("ContractTypeView :: Bytes"));
    }

    #[test]
    fn box_of_named_type_resolves_via_trait() {
        let ty: syn::Type = syn::parse_quote!(Box<Payload>);

        let view_tokens = contract_type_view_tokens(&ty).unwrap().to_string();

        assert!(
            view_tokens.contains("type_ref :: < Box < Payload > >"),
            "{view_tokens}"
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
