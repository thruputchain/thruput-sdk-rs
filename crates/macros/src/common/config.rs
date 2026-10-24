//! Parse attributes for the contract macro

use darling::FromMeta;
use darling::util::SpannedValue;
use proc_macro2::Span;
use quote::ToTokens;
use std::collections::BTreeMap;

/// One entry of a `throws(...)` list: the path naming an exception type.
///
/// A thrown type is just a type. It used to double as a variant name for
/// the generated client error enum, which is why differently-scoped types
/// sharing a last segment (`alpha::E` and `beta::E`) used to collide; with
/// no enum to name, they no longer do.
#[derive(Clone, Debug)]
pub struct ThrownType {
    ty: syn::Path,
}

impl ThrownType {
    fn from_path(path: &syn::Path) -> Self {
        Self { ty: path.clone() }
    }

    pub fn ty(&self) -> &syn::Path {
        &self.ty
    }

    fn ty_string(&self) -> String {
        self.ty.to_token_stream().to_string()
    }
}

#[derive(Clone, Debug)]
pub struct ThrowsList(Vec<ThrownType>);

impl ThrowsList {
    fn new(thrown_types: Vec<ThrownType>) -> darling::Result<Self> {
        if thrown_types.is_empty() {
            return Err(darling::Error::custom(
                "throws() requires at least one type",
            ));
        }

        // Same type listed twice: harmless to the client, but it would
        // duplicate the entry in the contract spec, and it is never what
        // the author meant.
        let mut seen = BTreeMap::new();
        for thrown in &thrown_types {
            if seen.insert(thrown.ty_string(), ()).is_some() {
                return Err(darling::Error::custom(format!(
                    "throws lists `{}` more than once",
                    thrown.ty_string(),
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
                Self::new(vec![ThrownType::from_path(&ep.path)])
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
                    thrown_types.push(ThrownType::from_path(path));
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
            &format!("__thruput_exports_{}", effective_interface_name),
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
    fn test_setup_key_is_rejected_as_unknown() {
        // Dead config key: darling's unknown-field error handles it
        // uniformly across item kinds and features.
        let tokens: proc_macro2::TokenStream =
            r#"test_setup = "setup_fn""#.parse().unwrap();
        let meta_list = NestedMeta::parse_meta_list(tokens).unwrap();
        let err = Config::from_list(&meta_list).unwrap_err();
        assert!(err.to_string().contains("test_setup"), "{err}");
    }

    // -- internal_exports_mod_ident --

    #[test]
    fn hidden_exports_name_from_effective_interface() {
        let config = empty_config();
        assert_eq!(
            config.internal_exports_mod_ident("Adder").to_string(),
            "__thruput_exports_Adder"
        );
        assert_eq!(
            config.internal_exports_mod_ident("ERC20").to_string(),
            "__thruput_exports_ERC20"
        );
        assert_eq!(
            config.internal_exports_mod_ident("ISwapV2Pair").to_string(),
            "__thruput_exports_ISwapV2Pair"
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
        assert_eq!(throws[0].ty().to_token_stream().to_string(), "MyError");
    }

    #[test]
    fn throws_list_syntax() {
        let config = parse_config(r#"throws(SwapError, ERC20Error)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].ty().to_token_stream().to_string(), "SwapError");
        assert_eq!(throws[1].ty().to_token_stream().to_string(), "ERC20Error");
    }

    #[test]
    fn throws_list_single() {
        let config = parse_config(r#"throws(SwapError)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].ty().to_token_stream().to_string(), "SwapError");
    }

    #[test]
    fn throws_path_single() {
        let config = parse_config(r#"throws = alpha::MyError"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
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
        assert_eq!(
            throws[0].ty().to_token_stream().to_string(),
            "alpha :: SwapError"
        );
        assert_eq!(
            throws[1].ty().to_token_stream().to_string(),
            "beta :: ERC20Error"
        );
    }

    /// Types that only share a last segment used to collide, because the
    /// last segment named a variant of the generated client error enum.
    #[test]
    fn throws_allows_same_named_types_from_different_modules() {
        let config = parse_config(r#"throws(alpha::E, beta::E)"#);
        assert_eq!(config.throws_list().len(), 2);
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
