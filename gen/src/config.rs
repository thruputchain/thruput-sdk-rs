//! Parse attributes for the contract macro

use darling::FromMeta;
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

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    pub test_setup: Option<String>,
    pub storage: Option<bool>,
    pub interface: Option<String>,
    pub version: Option<String>,
    pub doc: Option<String>,

    client: Option<syn::Ident>,
    throws: Option<ThrowsList>,
}

impl Config {
    pub fn client_ident(&self) -> Option<syn::Ident> {
        self.client.clone()
    }

    pub fn interface_name(&self) -> Option<String> {
        self.interface.clone()
    }

    pub fn internal_exports_mod_ident(
        &self,
        effective_interface_name: &str,
    ) -> syn::Ident {
        if let Some(name) = &self.exports {
            return syn::Ident::new(name, Span::call_site());
        }

        syn::Ident::new(
            &format!("__groundhog_exports_{}", effective_interface_name),
            Span::call_site(),
        )
    }

    pub fn storage(&self) -> bool {
        self.storage.unwrap_or(true)
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

    // -- storage --

    #[test]
    fn storage_defaults_to_true() {
        assert!(empty_config().storage());
    }

    #[test]
    fn storage_explicit_true() {
        assert!(parse_config(r#"storage = true"#).storage());
    }

    #[test]
    fn storage_explicit_false() {
        assert!(!parse_config(r#"storage = false"#).storage());
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
