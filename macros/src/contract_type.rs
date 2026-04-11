use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

pub fn expand(item: Item) -> TokenStream {
    match item {
        Item::Enum(item) => quote! {
            #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #[repr(i32)]
            #item
        },
        Item::Struct(item) => quote! {
            #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #item
        },
        other => syn::Error::new_spanned(
            other,
            "#[contracttype] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}
