use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ContractTypeArgs {
    pub name: Option<String>,
    pub export: Option<bool>,
}

pub fn expand(args: ContractTypeArgs, item: Item) -> TokenStream {
    match item {
        Item::Enum(item) => {
            if !item.generics.params.is_empty() {
                return syn::Error::new_spanned(
                    &item.generics,
                    "#[contracttype] is not supported on generic types",
                )
                .into_compile_error();
            }
            if let Err(err) = crate::check_enum_discriminants(&item.variants) {
                return err.into_compile_error();
            }
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let export_attr = if args.export == Some(true) {
                quote! { #[contract_type_spec(export)] }
            } else {
                quote! {}
            };
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #export_attr
                #item

                #name
            }
        }
        Item::Struct(item) => {
            if !item.generics.params.is_empty() {
                return syn::Error::new_spanned(
                    &item.generics,
                    "#[contracttype] is not supported on generic types",
                )
                .into_compile_error();
            }
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let export_attr = if args.export == Some(true) {
                quote! { #[contract_type_spec(export)] }
            } else {
                quote! {}
            };
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #export_attr
                #item

                #name
            }
        }
        other => syn::Error::new_spanned(
            other,
            "#[contracttype] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

fn expand_contract_type_name(
    ident: &syn::Ident,
    args: &ContractTypeArgs,
) -> TokenStream {
    crate::contract_type_name_impl(ident, args.name.as_deref())
}
