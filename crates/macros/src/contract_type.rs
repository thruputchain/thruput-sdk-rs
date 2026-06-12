use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
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
                    "#[contract_type] is not supported on generic types",
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
            let (preimage_mod, mintable_enum) =
                match generate_preimage_and_mintable_enum(&item) {
                    Ok(pair) => pair,
                    Err(err) => return err.into_compile_error(),
                };
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #export_attr
                #item

                #preimage_mod

                #name

                #collect_auth

                #receive_auth

                #mintable_enum
            }
        }
        Item::Struct(item) => {
            if !item.generics.params.is_empty() {
                return syn::Error::new_spanned(
                    &item.generics,
                    "#[contract_type] is not supported on generic types",
                )
                .into_compile_error();
            }
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let collect_auth = expand_collect_auth_handles_struct(&item);
            let receive_auth = expand_receive_auth_handles_struct(&item);
            let (preimage_mod, mintable) =
                match generate_preimage_and_mintable(&item) {
                    Ok(pair) => pair,
                    Err(err) => return err.into_compile_error(),
                };
            let export_attr = if args.export == Some(true) {
                quote! { #[contract_type_spec(export)] }
            } else {
                quote! {}
            };

            let spec_input =
                derive_input_from_item_struct(&item, &export_attr);
            let spec_tokens =
                match crate::derive_contract_type_spec::expand(&spec_input) {
                    Ok(tokens) => tokens,
                    Err(err) => return err.into_compile_error(),
                };

            // Reconstruct the struct from parts to avoid duplicating the
            // #[contract_type] attribute that syn already consumed.
            let user_attrs: Vec<_> = item.attrs.iter().collect();
            let vis = &item.vis;
            let fields = &item.fields;
            let semi = item.semi_token.map(|s| quote! { #s });

            let original_struct = match fields {
                syn::Fields::Named(_) => quote! {
                    #(#user_attrs)*
                    #vis struct #ident #fields
                },
                syn::Fields::Unnamed(_) => quote! {
                    #(#user_attrs)*
                    #vis struct #ident #fields #semi
                },
                syn::Fields::Unit => quote! {
                    #(#user_attrs)*
                    #vis struct #ident #semi
                },
            };

            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #original_struct

                #preimage_mod

                #name

                #spec_tokens

                #collect_auth

                #receive_auth

                #mintable
            }
        }
        other => syn::Error::new_spanned(
            other,
            "#[contract_type] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

/// Build a synthetic `DeriveInput` from the `ItemStruct` so that
/// `ContractTypeSpec` derive logic can run on the concrete field types.
fn derive_input_from_item_struct(
    item: &syn::ItemStruct,
    export_attr: &TokenStream,
) -> syn::DeriveInput {
    let mut attrs = item.attrs.clone();
    // Add the export attribute if present so the derive picks it up.
    if !export_attr.is_empty() {
        attrs.push(syn::parse_quote!(#[contract_type_spec(export)]));
    }
    syn::DeriveInput {
        attrs,
        vis: item.vis.clone(),
        ident: item.ident.clone(),
        generics: item.generics.clone(), // empty — original has no generics
        data: syn::Data::Struct(syn::DataStruct {
            struct_token: item.struct_token,
            fields: item.fields.clone(),
            semi_token: item.semi_token,
        }),
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
                quote! { self.#name.collect_auth_handles(out)?; }
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx.collect_auth_handles(out)?; }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    quote! {
        impl ::sdk::CollectAuthHandles for #ident {
            fn collect_auth_handles(
                &self,
                out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
            ) -> Result<(), ::sdk::FaultString> {
                #(#field_calls)*
                Ok(())
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
                quote! { self.#name.receive_auth_handles(handles)?; }
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx.receive_auth_handles(handles)?; }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    quote! {
        impl ::sdk::ReceiveAuthHandles for #ident {
            fn receive_auth_handles(
                &mut self,
                handles: &mut impl ::core::iter::Iterator<Item = u32>,
            ) -> Result<(), ::sdk::AuthTokenError> {
                #(#field_calls)*
                Ok(())
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
                        .map(|n| quote! { #n.receive_auth_handles(handles)?; })
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
                        .map(|b| quote! { #b.receive_auth_handles(handles)?; })
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
            ) -> Result<(), ::sdk::AuthTokenError> {
                match self {
                    #(#arms,)*
                }
                Ok(())
            }
        }
    }
}

/// Generate a `#[doc(hidden)]` companion module containing a `Preimage`
/// struct, plus `MintableType` and `Mintable` impls for the original struct.
///
/// Each field `T` in the Preimage becomes
/// `<T as ::sdk::mintable::MintableType>::Preimage`.
///
/// Returns `(preimage_mod_tokens, mintable_impl_tokens)`.
fn generate_preimage_and_mintable(
    item: &syn::ItemStruct,
) -> syn::Result<(TokenStream, TokenStream)> {
    let ident = &item.ident;
    let vis = &item.vis;
    let mod_name = format_ident!("__preimage_{}", ident);

    match &item.fields {
        syn::Fields::Named(fields) => {
            let preimage_fields: Vec<TokenStream> = fields
                .named
                .iter()
                .map(|f| {
                    let field_vis = &f.vis;
                    let name = &f.ident;
                    let ty = &f.ty;
                    quote! {
                        #field_vis #name: <#ty as ::sdk::mintable::MintableType>::Preimage
                    }
                })
                .collect();

            let preimage_mod = quote! {
                #[doc(hidden)]
                #[allow(non_snake_case)]
                #vis mod #mod_name {
                    use super::*;

                    #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
                    #[xdrs(crate = "::sdk::__xdrs")]
                    pub struct Preimage {
                        #(#preimage_fields,)*
                    }
                }
            };

            let mint_fields: Vec<TokenStream> = fields
                .named
                .iter()
                .map(|f| {
                    let name = &f.ident;
                    let ty = &f.ty;
                    quote! {
                        #name: <#ty as ::sdk::Mintable>::mint_into_parent(
                            preimage.#name, children,
                        )
                    }
                })
                .collect();

            let mintable_impl: TokenStream = quote! {
                impl ::sdk::mintable::MintableType for #ident {
                    type Preimage = #mod_name::Preimage;
                }

                impl ::sdk::Mintable for #ident {
                    fn mint_into_parent(
                        preimage: Self::Preimage,
                        children: &mut ::sdk::mintable::ChildNodeBuf,
                    ) -> Self {
                        Self {
                            #(#mint_fields,)*
                        }
                    }
                }
            };

            Ok((preimage_mod, mintable_impl))
        }
        syn::Fields::Unnamed(fields) => {
            let preimage_fields: Vec<TokenStream> = fields
                .unnamed
                .iter()
                .map(|f| {
                    let field_vis = &f.vis;
                    let ty = &f.ty;
                    quote! {
                        #field_vis <#ty as ::sdk::mintable::MintableType>::Preimage
                    }
                })
                .collect();

            let preimage_mod = quote! {
                #[doc(hidden)]
                #[allow(non_snake_case)]
                #vis mod #mod_name {
                    use super::*;

                    #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
                    #[xdrs(crate = "::sdk::__xdrs")]
                    pub struct Preimage(
                        #(#preimage_fields,)*
                    );
                }
            };

            let mint_fields: Vec<TokenStream> = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let idx = syn::Index::from(i);
                    let ty = &f.ty;
                    quote! {
                        <#ty as ::sdk::Mintable>::mint_into_parent(
                            preimage.#idx, children,
                        )
                    }
                })
                .collect();

            let mintable_impl = quote! {
                impl ::sdk::mintable::MintableType for #ident {
                    type Preimage = #mod_name::Preimage;
                }

                impl ::sdk::Mintable for #ident {
                    fn mint_into_parent(
                        preimage: Self::Preimage,
                        children: &mut ::sdk::mintable::ChildNodeBuf,
                    ) -> Self {
                        Self(
                            #(#mint_fields,)*
                        )
                    }
                }
            };

            Ok((preimage_mod, mintable_impl))
        }
        syn::Fields::Unit => {
            let preimage_mod = quote! {
                #[doc(hidden)]
                #[allow(non_snake_case)]
                #vis mod #mod_name {
                    #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
                    #[xdrs(crate = "::sdk::__xdrs")]
                    pub struct Preimage;
                }
            };

            let mintable_impl = quote! {
                impl ::sdk::mintable::MintableType for #ident {
                    type Preimage = #mod_name::Preimage;
                }

                impl ::sdk::Mintable for #ident {
                    fn mint_into_parent(
                        _preimage: Self::Preimage,
                        _children: &mut ::sdk::mintable::ChildNodeBuf,
                    ) -> Self {
                        Self
                    }
                }
            };

            Ok((preimage_mod, mintable_impl))
        }
    }
}

/// Enum counterpart of [`generate_preimage_and_mintable`]: a mirrored
/// `Preimage` enum (same variants, same explicit discriminants, every
/// field type `T` replaced by `<T as MintableType>::Preimage`) plus
/// `MintableType`/`Mintable` impls whose `mint_into_parent` recurses
/// into each variant's fields, so `Signed` fields in enum payloads mint
/// child auth-token nodes exactly like struct fields do.
fn generate_preimage_and_mintable_enum(
    item: &syn::ItemEnum,
) -> syn::Result<(TokenStream, TokenStream)> {
    let ident = &item.ident;
    let vis = &item.vis;
    let mod_name = format_ident!("__preimage_{}", ident);

    let mut preimage_variants: Vec<TokenStream> = Vec::new();
    let mut mint_arms: Vec<TokenStream> = Vec::new();

    for variant in &item.variants {
        let vident = &variant.ident;
        let Some((_, disc)) = variant.discriminant.as_ref() else {
            return Err(syn::Error::new_spanned(
                variant,
                "enum variants must use explicit discriminants for contract specs",
            ));
        };

        match &variant.fields {
            syn::Fields::Named(fields) => {
                let pre_fields: Vec<TokenStream> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote! {
                            #name: <#ty as ::sdk::mintable::MintableType>::Preimage
                        }
                    })
                    .collect();
                preimage_variants.push(quote! {
                    #vident { #(#pre_fields),* } = #disc
                });

                let bindings: Vec<syn::Ident> = fields
                    .named
                    .iter()
                    .map(|f| {
                        format_ident!(
                            "__field_{}",
                            f.ident.as_ref().expect("named field has ident")
                        )
                    })
                    .collect();
                let pattern_fields: Vec<TokenStream> = fields
                    .named
                    .iter()
                    .zip(&bindings)
                    .map(|(f, b)| {
                        let name = &f.ident;
                        quote! { #name: #b }
                    })
                    .collect();
                let mints: Vec<TokenStream> = fields
                    .named
                    .iter()
                    .zip(&bindings)
                    .map(|(f, b)| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote! {
                            #name: <#ty as ::sdk::Mintable>::mint_into_parent(
                                #b, children,
                            )
                        }
                    })
                    .collect();
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident { #(#pattern_fields),* } =>
                        Self::#vident { #(#mints),* }
                });
            }
            syn::Fields::Unnamed(fields) => {
                let pre_fields: Vec<TokenStream> = fields
                    .unnamed
                    .iter()
                    .map(|f| {
                        let ty = &f.ty;
                        quote! {
                            <#ty as ::sdk::mintable::MintableType>::Preimage
                        }
                    })
                    .collect();
                preimage_variants.push(quote! {
                    #vident(#(#pre_fields),*) = #disc
                });

                let bindings: Vec<syn::Ident> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("f{}", i))
                    .collect();
                let mints: Vec<TokenStream> = fields
                    .unnamed
                    .iter()
                    .zip(&bindings)
                    .map(|(f, b)| {
                        let ty = &f.ty;
                        quote! {
                            <#ty as ::sdk::Mintable>::mint_into_parent(
                                #b, children,
                            )
                        }
                    })
                    .collect();
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident(#(#bindings),*) =>
                        Self::#vident(#(#mints),*)
                });
            }
            syn::Fields::Unit => {
                preimage_variants.push(quote! { #vident = #disc });
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident => Self::#vident
                });
            }
        }
    }

    let preimage_mod = quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #vis mod #mod_name {
            use super::*;

            #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #[repr(i32)]
            pub enum Preimage {
                #(#preimage_variants,)*
            }
        }
    };

    let mintable_impl = quote! {
        impl ::sdk::mintable::MintableType for #ident {
            type Preimage = #mod_name::Preimage;
        }

        impl ::sdk::Mintable for #ident {
            fn mint_into_parent(
                preimage: Self::Preimage,
                children: &mut ::sdk::mintable::ChildNodeBuf,
            ) -> Self {
                match preimage {
                    #(#mint_arms,)*
                }
            }
        }
    };

    Ok((preimage_mod, mintable_impl))
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
                        .map(|n| quote! { #n.collect_auth_handles(out)?; })
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
                        .map(|b| quote! { #b.collect_auth_handles(out)?; })
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
            ) -> Result<(), ::sdk::FaultString> {
                match self {
                    #(#arms,)*
                }
                Ok(())
            }
        }
    }
}
