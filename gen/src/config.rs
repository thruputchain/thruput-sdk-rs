//! Parse attributes for the contract macro

use darling::FromMeta;
use darling::util::Flag;
use proc_macro2::Span;

#[derive(Clone, Debug)]
pub struct ThrowsList(Vec<syn::Ident>);

impl FromMeta for ThrowsList {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        Err(darling::Error::unexpected_lit_type(value))
    }

    fn from_word() -> darling::Result<Self> {
        Err(darling::Error::unsupported_format("word"))
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(ep) if ep.path.get_ident().is_some() => {
                let ident = ep.path.get_ident().unwrap().clone();
                Ok(ThrowsList(vec![ident]))
            }
            _ => Err(darling::Error::custom(
                "throws expects an identifier or list of identifiers",
            )),
        }
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut idents = Vec::new();
        for item in items {
            match item {
                darling::ast::NestedMeta::Meta(syn::Meta::Path(path)) => {
                    if let Some(ident) = path.get_ident() {
                        idents.push(ident.clone());
                    } else {
                        return Err(
                            darling::Error::custom(
                                "throws expects simple identifiers, not paths",
                            )
                            .with_span(path),
                        );
                    }
                }
                _ => return Err(darling::Error::custom("throws expects identifiers")),
            }
        }
        if idents.is_empty() {
            return Err(darling::Error::custom(
                "throws() requires at least one type",
            ));
        }
        Ok(ThrowsList(idents))
    }
}

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    pub test_setup: Option<String>,
    pub storage: Option<bool>,
    pub interface: Option<String>,
    pub version: Option<String>,
    pub init: Flag,
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

    pub fn exports_mod_ident(&self) -> syn::Ident {
        ident_or(&self.exports, "exports")
    }

    /// Returns the exports module identifier.
    /// When `trait_name` is provided and no explicit `exports` was set,
    /// derives the name from the trait (e.g., `Adder` → `Adder_exports`).
    pub fn exports_mod_ident_for_trait(
        &self,
        trait_name: Option<&str>,
    ) -> syn::Ident {
        match (&self.exports, trait_name) {
            (Some(name), _) => syn::Ident::new(name, Span::call_site()),
            (None, Some(name)) => {
                syn::Ident::new(&format!("{name}_exports"), Span::call_site())
            }
            (None, None) => syn::Ident::new("exports", Span::call_site()),
        }
    }

    pub fn storage(&self) -> bool {
        self.storage.unwrap_or(true)
    }

    /// Returns the list of thrown error types (empty if none declared).
    pub fn throws_list(&self) -> Vec<&syn::Ident> {
        match &self.throws {
            Some(ThrowsList(idents)) => idents.iter().collect(),
            None => vec![],
        }
    }

    /// Returns the first thrown error type (backwards compat).
    pub fn throws(&self) -> Option<&syn::Ident> {
        self.throws.as_ref().and_then(|t| t.0.first())
    }
}

fn ident_or(maybe_name: &Option<String>, default: &str) -> syn::Ident {
    let span = Span::call_site();
    match maybe_name {
        Some(name_string) => syn::Ident::new(name_string.as_ref(), span),
        None => syn::Ident::new(default, span),
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

    // -- exports_mod_ident_for_trait --

    #[test]
    fn exports_name_from_trait() {
        let config = empty_config();
        assert_eq!(
            config
                .exports_mod_ident_for_trait(Some("Adder"))
                .to_string(),
            "Adder_exports"
        );
        assert_eq!(
            config
                .exports_mod_ident_for_trait(Some("ERC20"))
                .to_string(),
            "ERC20_exports"
        );
        assert_eq!(
            config
                .exports_mod_ident_for_trait(Some("ISwapV2Pair"))
                .to_string(),
            "ISwapV2Pair_exports"
        );
    }

    #[test]
    fn exports_name_defaults_without_trait() {
        let config = empty_config();
        assert_eq!(
            config.exports_mod_ident_for_trait(None).to_string(),
            "exports"
        );
    }

    #[test]
    fn explicit_exports_overrides_trait_name() {
        let config = parse_config(r#"exports = "custom""#);
        assert_eq!(
            config
                .exports_mod_ident_for_trait(Some("Adder"))
                .to_string(),
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
        assert_eq!(throws[0].to_string(), "MyError");
    }

    #[test]
    fn throws_list_syntax() {
        let config = parse_config(r#"throws(SwapError, ERC20Error)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].to_string(), "SwapError");
        assert_eq!(throws[1].to_string(), "ERC20Error");
    }

    #[test]
    fn throws_list_single() {
        let config = parse_config(r#"throws(SwapError)"#);
        let throws = config.throws_list();
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].to_string(), "SwapError");
    }

    #[test]
    fn throws_none() {
        let config = parse_config(r#""#);
        let throws = config.throws_list();
        assert!(throws.is_empty());
    }
}
