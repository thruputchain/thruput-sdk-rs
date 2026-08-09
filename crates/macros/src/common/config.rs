//! Parse attributes for the contract macro

use darling::FromMeta;
use darling::util::SpannedValue;
use proc_macro2::Span;
use quote::ToTokens;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ThrownType {
    ty: syn::Path,
    variant: syn::Ident,
}

impl ThrownType {
    fn from_path(path: &syn::Path) -> darling::Result<Self> {
        let variant = path
            .segments
            .last()
            .map(|seg| seg.ident.clone())
            .ok_or_else(|| {
                darling::Error::custom("throws expects a type path")
                    .with_span(path)
            })?;

        Ok(Self {
            ty: path.clone(),
            variant,
        })
    }

    pub fn ty(&self) -> &syn::Path {
        &self.ty
    }

    pub fn variant(&self) -> &syn::Ident {
        &self.variant
    }

    fn ty_string(&self) -> String {
        self.ty.to_token_stream().to_string()
    }
}

#[derive(Clone, Debug)]
pub struct ThrowsList(Vec<ThrownType>);

impl ThrowsList {
    fn new(thrown_types: Vec<ThrownType>) -> darling::Result<Self> {
        const RESERVED: &[&str] = &["Library", "Fault", "Unknown"];

        if thrown_types.is_empty() {
            return Err(darling::Error::custom(
                "throws() requires at least one type",
            ));
        }

        let mut seen_variants = BTreeMap::new();
        for thrown in &thrown_types {
            let variant_name = thrown.variant.to_string();
            if RESERVED.contains(&variant_name.as_str()) {
                return Err(darling::Error::custom(format!(
                    "`{}` is a reserved error variant name \
                     (used by the generated client enum)",
                    thrown.variant
                ))
                .with_span(thrown.variant()));
            }

            if let Some(prev_ty) =
                seen_variants.insert(variant_name.clone(), thrown.ty_string())
            {
                return Err(darling::Error::custom(format!(
                    "throws contains duplicate generated client error variant `{}`; `{}` and `{}` would both map to it",
                    variant_name,
                    prev_ty,
                    thrown.ty_string()
                ))
                .with_span(thrown.ty()));
            }
        }

        Ok(Self(thrown_types))
    }
}

impl FromMeta for ThrowsList {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        Err(darling::Error::unexpected_lit_type(value))
    }

    fn from_word() -> darling::Result<Self> {
        Err(darling::Error::unsupported_format("word"))
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(ep) if ep.qself.is_none() => {
                Self::new(vec![ThrownType::from_path(&ep.path)?])
            }
            _ => Err(darling::Error::custom(
                "throws expects a type path or list of type paths",
            )),
        }
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut thrown_types = Vec::new();
        for item in items {
            match item {
                darling::ast::NestedMeta::Meta(syn::Meta::Path(path)) => {
                    thrown_types.push(ThrownType::from_path(path)?);
                }
                _ => {
                    return Err(darling::Error::custom(
                        "throws expects type paths",
                    ));
                }
            }
        }
        Self::new(thrown_types)
    }
}

/// A list of event type paths declared via `emits(...)`.
#[derive(Clone, Debug)]
pub struct EmitsList(Vec<syn::Path>);

impl EmitsList {
    fn new(events: Vec<syn::Path>) -> darling::Result<Self> {
        if events.is_empty() {
            return Err(darling::Error::custom(
                "emits() requires at least one event type",
            ));
        }
        Ok(Self(events))
    }
}

impl FromMeta for EmitsList {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        Err(darling::Error::unexpected_lit_type(value))
    }

    fn from_word() -> darling::Result<Self> {
        Err(darling::Error::unsupported_format("word"))
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(ep) if ep.qself.is_none() => {
                Self::new(vec![ep.path.clone()])
            }
            _ => Err(darling::Error::custom(
                "emits expects a type path or list of type paths",
            )),
        }
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut events = Vec::new();
        for item in items {
            match item {
                darling::ast::NestedMeta::Meta(syn::Meta::Path(path)) => {
                    events.push(path.clone());
                }
                _ => {
                    return Err(darling::Error::custom(
                        "emits expects type paths",
                    ));
                }
            }
        }
        Self::new(events)
    }
}

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<SpannedValue<String>>,
    /// Dead key, kept only so `validate` can reject it with a migration
    /// hint instead of darling's bare unknown-field error.
    pub test_setup: Option<SpannedValue<String>>,
    pub interface: Option<SpannedValue<String>>,
    pub version: Option<SpannedValue<String>>,

    client: Option<syn::Ident>,
    throws: Option<ThrowsList>,
    emits: Option<EmitsList>,
}

impl Config {
    /// Validate user-supplied string attributes up front, so malformed
    /// input produces a compile error instead of a proc-macro panic
    /// deeper in code generation.
    pub fn validate(&self) -> syn::Result<()> {
        use super::util::{validate_ident_string, validate_version_string};

        if let Some(version) = &self.version {
            validate_version_string(version.span(), version)?;
        }
        if let Some(exports) = &self.exports {
            validate_ident_string(exports.span(), exports, "exports")?;
        }
        if let Some(interface) = &self.interface {
            validate_ident_string(interface.span(), interface, "interface")?;
        }
        // Rejected uniformly here (rather than dropped from the struct) so
        // the error carries the migration hint on every item kind and
        // feature combination — historically it was silently ignored on
        // traits/free functions and on builds without the mock feature.
        if let Some(test_setup) = &self.test_setup {
            return Err(syn::Error::new(
                test_setup.span(),
                "test_setup = \"...\" is no longer supported: #[contract] \
                 generates `__mock` automatically; use \
                 `<Type as Trait>::__mock` (or `Type::__mock` for bare \
                 impls) in tests instead",
            ));
        }
        Ok(())
    }

    pub fn client_ident(&self) -> Option<syn::Ident> {
        self.client.clone()
    }

    pub fn interface_name(&self) -> Option<String> {
        self.interface.as_ref().map(|s| s.as_str().to_string())
    }

    pub fn version_string(&self) -> Option<String> {
        self.version.as_ref().map(|v| v.as_str().to_string())
    }

    pub fn internal_exports_mod_ident(
        &self,
        effective_interface_name: &str,
    ) -> syn::Ident {
        if let Some(name) = &self.exports {
            return syn::Ident::new(name, name.span());
        }

        syn::Ident::new(
            &format!("__groundhog_exports_{}", effective_interface_name),
            Span::call_site(),
        )
    }

    /// Returns the list of thrown error types (empty if none declared).
    pub fn throws_list(&self) -> Vec<ThrownType> {
        match &self.throws {
            Some(ThrowsList(thrown_types)) => thrown_types.clone(),
            None => vec![],
        }
    }

    /// Returns the first thrown error type (backwards compat).
    pub fn throws(&self) -> Option<ThrownType> {
        self.throws.as_ref().and_then(|t| t.0.first()).cloned()
    }

    /// Returns the list of declared event types (empty if none declared).
    pub fn emits_list(&self) -> Vec<syn::Path> {
        match &self.emits {
            Some(EmitsList(events)) => events.clone(),
            None => vec![],
        }
    }

    /// Returns true if any events were declared via `emits(...)`.
    pub fn has_emits(&self) -> bool {
        self.emits.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use darling::ast::NestedMeta;

    /// Parse a `Config` from a `#[contract(...)]`-style attribute string.
    fn parse_config(attrs: &str) -> Config {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list).unwrap()
    }

    fn empty_config() -> Config {
        parse_config("")
    }

    #[test]
    fn test_setup_key_is_rejected_with_migration_hint() {
        // Dead config key, previously ignored or erratically rejected
        // depending on feature and item kind. Now rejected uniformly in
        // Config::validate with a hint at the __mock replacement, so a
        // crate migrating from an old SDK learns where the feature went.
        let tokens: proc_macro2::TokenStream =
            r#"test_setup = "setup_fn""#.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        let config = Config::from_list(&meta_list).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("__mock"), "{err}");
    }

    // -- internal_exports_mod_ident --

    #[test]
    fn hidden_exports_name_from_effective_interface() {
        let config = empty_config();
        assert_eq!(
            config.internal_exports_mod_ident("Adder").to_string(),
            "__groundhog_exports_Adder"
        );
        assert_eq!(
            config.internal_exports_mod_ident("ERC20").to_string(),
            "__groundhog_exports_ERC20"
        );
        assert_eq!(
            config.internal_exports_mod_ident("ISwapV2Pair").to_string(),
            "__groundhog_exports_ISwapV2Pair"
        );
    }

    #[test]
    fn explicit_exports_overrides_hidden_exports_name() {
        let config = parse_config(r#"exports = "custom""#);
        assert_eq!(
            config.internal_exports_mod_ident("Adder").to_string(),
            "custom"
        );
    }

    // -- throws --

    #[test]
    fn throws_single_ident() {
        let config = parse_config(r#"throws = MyError"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].variant().to_string(), "MyError");
        assert_eq!(throws[0].ty().to_token_stream().to_string(), "MyError");
    }

    #[test]
    fn throws_list_syntax() {
        let config = parse_config(r#"throws(SwapError, ERC20Error)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].variant().to_string(), "SwapError");
        assert_eq!(throws[0].ty().to_token_stream().to_string(), "SwapError");
        assert_eq!(throws[1].variant().to_string(), "ERC20Error");
        assert_eq!(throws[1].ty().to_token_stream().to_string(), "ERC20Error");
    }

    #[test]
    fn throws_list_single() {
        let config = parse_config(r#"throws(SwapError)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].variant().to_string(), "SwapError");
    }

    #[test]
    fn throws_path_single() {
        let config = parse_config(r#"throws = alpha::MyError"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].variant().to_string(), "MyError");
        assert_eq!(
            throws[0].ty().to_token_stream().to_string(),
            "alpha :: MyError"
        );
    }

    #[test]
    fn throws_path_list_syntax() {
        let config =
            parse_config(r#"throws(alpha::SwapError, beta::ERC20Error)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].variant().to_string(), "SwapError");
        assert_eq!(throws[1].variant().to_string(), "ERC20Error");
    }

    #[test]
    fn throws_none() {
        let config = parse_config(r#""#);
        let throws = config.throws_list();
        assert!(throws.is_empty());
    }
}

#[cfg(test)]
mod emits_tests {
    use super::*;
    use darling::ast::NestedMeta;

    fn parse_config(attrs: &str) -> darling::Result<Config> {
        let tokens: proc_macro2::TokenStream = attrs.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        Config::from_list(&meta_list)
    }

    #[test]
    fn emits_list_syntax() {
        let config =
            parse_config(r#"emits(TransferEvent, events::ApprovalEvent)"#)
                .unwrap();
        let emits = config.emits_list();
        assert_eq!(emits.len(), 2);
        assert_eq!(emits[0].to_token_stream().to_string(), "TransferEvent");
        assert_eq!(
            emits[1].to_token_stream().to_string(),
            "events :: ApprovalEvent"
        );
    }

    #[test]
    fn emits_single_path() {
        let config = parse_config(r#"emits = TransferEvent"#).unwrap();
        assert_eq!(config.emits_list().len(), 1);
        assert!(config.has_emits());
    }

    #[test]
    fn emits_none() {
        let config = parse_config(r#""#).unwrap();
        assert!(config.emits_list().is_empty());
        assert!(!config.has_emits());
    }

    #[test]
    fn emits_empty_list_rejected() {
        assert!(parse_config(r#"emits()"#).is_err());
    }
}
