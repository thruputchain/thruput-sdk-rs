use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ContractTypeArgs {
    pub name: Option<String>,
}

pub fn expand(args: ContractTypeArgs, item: Item) -> TokenStream {
    match item {
        Item::Enum(item) => {
            if let Err(err) = crate::check_enum_discriminants(&item.variants) {
                return err.into_compile_error();
            }
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #item

                #name
            }
        }
        Item::Struct(item) => {
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
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
