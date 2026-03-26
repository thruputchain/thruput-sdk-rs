use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Expand `#[contract_error]` into the full derive set + `#[repr(i32)]`.
///
/// With `feature = "dev"` (in the consuming crate):
///   - Derives `thiserror::Error` for Display/Error impls
///   - Keeps `#[error("...")]` attributes on variants
///
/// Without `dev`:
///   - Does NOT derive `thiserror::Error`
///   - Strips `#[error("...")]` attributes so they don't cause errors
///   - Error strings are completely absent from the wasm binary
pub fn expand(item: Item) -> TokenStream {
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

    let mut item_stripped = item.clone();
    strip_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error, Exn)]
        #[repr(i32)]
        #item

        #[cfg(not(feature = "dev"))]
        #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn)]
        #[repr(i32)]
        #item_stripped
    }
}

/// Remove all `#[error(...)]` attributes from an enum's variants.
fn strip_error_attrs(item: &mut syn::ItemEnum) {
    for variant in &mut item.variants {
        variant.attrs.retain(|attr| !attr.path().is_ident("error"));
    }
}
