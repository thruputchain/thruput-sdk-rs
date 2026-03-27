use super::derive_exn::ExnArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, Item};

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the full derive set + `#[repr(i32)]` (enums only) + `impl Exn`.
///
/// Supported on enums and structs.
///
/// With `feature = "dev"` (in the consuming crate):
///   - Derives `thiserror::Error` for Display/Error impls
///   - Keeps `#[error("...")]` attributes on variants/structs
///
/// Without `dev`:
///   - Does NOT derive `thiserror::Error`
///   - Strips `#[error("...")]` attributes so they don't cause errors
///   - Error strings are completely absent from the wasm binary
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
    let type_name = item.ident.to_string();
    let ident = &item.ident;
    let selector = compute_selector(&args, &type_name);

    let mut item_stripped = item.clone();
    strip_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
        #[cfg_attr(feature = "dev", thiserror(crate = ::sdk::__private::thiserror))]
        #[repr(i32)]
        #item

        #[cfg(not(feature = "dev"))]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[repr(i32)]
        #item_stripped

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }
    }
}

fn expand_struct(args: ExnArgs, item: syn::ItemStruct) -> TokenStream {
    let type_name = item.ident.to_string();
    let ident = &item.ident;
    let selector = compute_selector(&args, &type_name);

    let mut item_stripped = item.clone();
    strip_struct_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
        #[cfg_attr(feature = "dev", thiserror(crate = ::sdk::__private::thiserror))]
        #item

        #[cfg(not(feature = "dev"))]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #item_stripped

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

/// Remove all `#[error(...)]` attributes from an enum's variants.
fn strip_error_attrs(item: &mut syn::ItemEnum) {
    for variant in &mut item.variants {
        variant.attrs.retain(|attr| !attr.path().is_ident("error"));
    }
}

/// Remove thiserror attributes from a struct:
/// - `#[error(...)]` from the struct's own attrs
/// - `#[source]` and `#[from]` from field attrs
fn strip_struct_error_attrs(item: &mut syn::ItemStruct) {
    item.attrs.retain(|attr| !attr.path().is_ident("error"));

    let fields = match &mut item.fields {
        Fields::Named(f) => f.named.iter_mut().collect::<Vec<_>>(),
        Fields::Unnamed(f) => f.unnamed.iter_mut().collect::<Vec<_>>(),
        Fields::Unit => vec![],
    };

    for field in fields {
        field.attrs.retain(|attr| {
            !attr.path().is_ident("source") && !attr.path().is_ident("from")
        });
    }
}
