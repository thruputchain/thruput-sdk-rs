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
            let collect_auth = expand_collect_auth_handles_enum(&item);
            let receive_auth = expand_receive_auth_handles_enum(&item);
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

                #collect_auth

                #receive_auth
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
            let collect_auth = expand_collect_auth_handles_struct(&item);
            let receive_auth = expand_receive_auth_handles_struct(&item);
            let mintable = expand_mintable_struct(&item);
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

                #collect_auth

                #receive_auth

                #mintable
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

fn expand_collect_auth_handles_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let field_calls: Vec<TokenStream> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let name = &f.ident;
                quote! { self.#name.collect_auth_handles(out); }
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx.collect_auth_handles(out); }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    quote! {
        impl ::sdk::CollectAuthHandles for #ident {
            fn collect_auth_handles(
                &self,
                out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
            ) {
                #(#field_calls)*
            }
        }
    }
}

fn expand_receive_auth_handles_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let field_calls: Vec<TokenStream> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let name = &f.ident;
                quote! { self.#name.receive_auth_handles(handles); }
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx.receive_auth_handles(handles); }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    quote! {
        impl ::sdk::ReceiveAuthHandles for #ident {
            fn receive_auth_handles(
                &mut self,
                handles: &mut impl ::core::iter::Iterator<Item = u32>,
            ) {
                #(#field_calls)*
            }
        }
    }
}

fn expand_receive_auth_handles_enum(item: &syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let arms: Vec<TokenStream> = item
        .variants
        .iter()
        .map(|variant| {
            let vident = &variant.ident;
            match &variant.fields {
                syn::Fields::Named(fields) => {
                    let names: Vec<_> =
                        fields.named.iter().map(|f| &f.ident).collect();
                    let calls: Vec<TokenStream> = names
                        .iter()
                        .map(|n| quote! { #n.receive_auth_handles(handles); })
                        .collect();
                    quote! {
                        Self::#vident { #(#names),* } => { #(#calls)* }
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    let bindings: Vec<syn::Ident> = (0..fields.unnamed.len())
                        .map(|i| quote::format_ident!("f{}", i))
                        .collect();
                    let calls: Vec<TokenStream> = bindings
                        .iter()
                        .map(|b| quote! { #b.receive_auth_handles(handles); })
                        .collect();
                    quote! {
                        Self::#vident(#(#bindings),*) => { #(#calls)* }
                    }
                }
                syn::Fields::Unit => {
                    quote! { Self::#vident => {} }
                }
            }
        })
        .collect();

    quote! {
        impl ::sdk::ReceiveAuthHandles for #ident {
            fn receive_auth_handles(
                &mut self,
                handles: &mut impl ::core::iter::Iterator<Item = u32>,
            ) {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

fn expand_mintable_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let unsigned_ident = quote::format_ident!("{}Unsigned", ident);
    let snake = to_snake_case(&ident.to_string());
    let mintable_mod_ident =
        quote::format_ident!("__groundhog_mintable_{}", snake);

    let fields = match &item.fields {
        syn::Fields::Named(fields) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                item,
                "Mintable derive requires named fields",
            )
            .into_compile_error();
        }
    };

    let unsigned_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote! { pub #name: <#ty as ::sdk::Mintable>::Unsigned }
        })
        .collect();

    let mint_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote! {
                #name: <#ty as ::sdk::Mintable>::mint_into_parent(
                    unsigned.#name, children,
                )
            }
        })
        .collect();

    quote! {
        #[cfg(feature = "alloc")]
        mod #mintable_mod_ident {
            #[allow(unused_imports)]
            use super::*;

            pub struct #unsigned_ident {
                #(#unsigned_fields,)*
            }
        }

        #[cfg(feature = "alloc")]
        impl ::sdk::Mintable for #ident {
            type Unsigned = #mintable_mod_ident::#unsigned_ident;

            fn mint_into_parent(
                unsigned: Self::Unsigned,
                children: &mut ::sdk::Vec<::sdk::AuthTokenNode>,
            ) -> Self {
                Self {
                    #(#mint_fields,)*
                }
            }
        }
    }
}

fn expand_collect_auth_handles_enum(item: &syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let arms: Vec<TokenStream> = item
        .variants
        .iter()
        .map(|variant| {
            let vident = &variant.ident;
            match &variant.fields {
                syn::Fields::Named(fields) => {
                    let names: Vec<_> =
                        fields.named.iter().map(|f| &f.ident).collect();
                    let calls: Vec<TokenStream> = names
                        .iter()
                        .map(|n| quote! { #n.collect_auth_handles(out); })
                        .collect();
                    quote! {
                        Self::#vident { #(#names),* } => { #(#calls)* }
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    let bindings: Vec<syn::Ident> = (0..fields.unnamed.len())
                        .map(|i| quote::format_ident!("f{}", i))
                        .collect();
                    let calls: Vec<TokenStream> = bindings
                        .iter()
                        .map(|b| quote! { #b.collect_auth_handles(out); })
                        .collect();
                    quote! {
                        Self::#vident(#(#bindings),*) => { #(#calls)* }
                    }
                }
                syn::Fields::Unit => {
                    quote! { Self::#vident => {} }
                }
            }
        })
        .collect();

    quote! {
        impl ::sdk::CollectAuthHandles for #ident {
            fn collect_auth_handles(
                &self,
                out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
            ) {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}
