use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Expand `#[contract_error]` into the full derive set + `#[repr(i32)]`.
///
/// Always derives: `Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error, Exn`
/// Always adds: `#[repr(i32)]`
///
/// Users write `#[error("...")]` on variants as normal.
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

    quote! {
        #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error, Exn)]
        #[repr(i32)]
        #item
    }
}
