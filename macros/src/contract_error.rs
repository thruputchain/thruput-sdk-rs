use super::derive_exn::ExnArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the full derive set + `#[repr(i32)]` + `impl Exn`.
///
/// With `feature = "dev"` (in the consuming crate):
///   - Derives `thiserror::Error` for Display/Error impls
///   - Keeps `#[error("...")]` attributes on variants
///
/// Without `dev`:
///   - Does NOT derive `thiserror::Error`
///   - Strips `#[error("...")]` attributes so they don't cause errors
///   - Error strings are completely absent from the wasm binary
pub fn expand(args: ExnArgs, item: Item) -> TokenStream {
    let item = match item {
        Item::Enum(e) => e,
        Item::Struct(s) => {
            return syn::Error::new_spanned(
                s.struct_token,
                "#[contract_error] is only supported on enums",
            )
            .into_compile_error();
        }
        other => {
            return syn::Error::new_spanned(
                other,
                "#[contract_error] is only supported on enums",
            )
            .into_compile_error();
        }
    };

    let type_name = item.ident.to_string();
    let ident = &item.ident;

    let version = args.version.unwrap_or_else(|| {
        std::env::var("CARGO_PKG_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string())
    });
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());

    let selector = groundhog_contract_util::error_selector(
        &version, &crate_name, &type_name,
    );

    let mut item_stripped = item.clone();
    strip_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, thiserror::Error)]
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

/// Remove all `#[error(...)]` attributes from an enum's variants.
fn strip_error_attrs(item: &mut syn::ItemEnum) {
    for variant in &mut item.variants {
        variant
            .attrs
            .retain(|attr| !attr.path().is_ident("error"));
    }
}
