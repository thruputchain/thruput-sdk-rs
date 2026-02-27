use crate::config::Config;
use syn::parse_quote;

pub fn throws(config: &Config) -> syn::TypeParamBound {
    if let Some(e) = config.throws() {
        parse_quote! {
            Throws<Error = #e>
        }
    } else {
        parse_quote! {
            Throws<Error = ::core::convert::Infallible>
        }
    }
}
