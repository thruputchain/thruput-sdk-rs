use super::derive_exn::ExnArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the standard derive set + `#[repr(i32)]` (enums only) + `impl Exn`.
///
/// Supported on enums and structs. All user-supplied attributes (including
/// `#[error("...")]` from thiserror) are passed through unchanged.
pub fn expand(args: ExnArgs, item: Item) -> TokenStream {
    match item {
        Item::Enum(e) => expand_enum(args, e),
        Item::Struct(s) => expand_struct(args, s),
        other => syn::Error::new_spanned(
            other,
            "#[contract_error] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

fn expand_enum(args: ExnArgs, item: syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }
    }
}

fn expand_struct(args: ExnArgs, item: syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }
    }
}

fn compute_selector(args: &ExnArgs, type_name: &str) -> u64 {
    let version = args.version.clone().unwrap_or_else(|| {
        std::env::var("CARGO_PKG_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string())
    });
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());
    groundhog_contract_util::error_selector(&version, &crate_name, type_name)
}
