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
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            quote! {
                #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
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
                #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
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
    let name_value = match &args.name {
        Some(custom) => quote! { #custom },
        None => quote! { stringify!(#ident) },
    };
    quote! {
        impl ::sdk::__contract_spec::ContractTypeName for #ident {
            const NAME: &'static str = #name_value;
        }
    }
}
