use super::derive_exn::ExnArgs;
use groundhog_codegen as codegen;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the standard derive set + `#[repr(i32)]` (enums only) + `impl Exn`.
///
/// Supported on enums and structs. All user-supplied attributes (including
/// `#[error("...")]` from thiserror) are passed through unchanged.
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
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let type_def = match enum_type_def_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let type_defs = match enum_type_defs_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let alloc_marker_impl = match enum_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let raw_typedef = emit_raw_typedef_fragment(ident);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #alloc_marker_impl

        #raw_typedef
    }
}

fn expand_struct(args: ExnArgs, item: syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let type_def = match struct_type_def_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let type_defs = match struct_type_defs_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let alloc_marker_impl = match struct_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let raw_typedef = emit_raw_typedef_fragment(ident);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #alloc_marker_impl

        #raw_typedef
    }
}

fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    codegen::spec_reg::emit_raw_typedef_fragment_stream_tokens_with_cfg(
        quote! { target_family = "wasm" },
        quote! { <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEFS },
    )
}

fn struct_type_defs_tokens(
    item: &syn::ItemStruct,
) -> syn::Result<TokenStream> {
    let nested = fields_type_defs_tokens(&item.fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            ::sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
            #(#nested),*
        ])
    })
}

fn struct_alloc_enabled_impl_tokens(
    item: &syn::ItemStruct,
) -> syn::Result<TokenStream> {
    alloc_enabled_impl_tokens(&item.ident, field_types(&item.fields)?)
}

fn enum_type_defs_tokens(item: &syn::ItemEnum) -> syn::Result<TokenStream> {
    let nested = item
        .variants
        .iter()
        .map(enum_variant_type_defs_tokens)
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            ::sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
            #(#nested),*
        ])
    })
}

fn enum_alloc_enabled_impl_tokens(
    item: &syn::ItemEnum,
) -> syn::Result<TokenStream> {
    let mut tys = Vec::new();
    for variant in &item.variants {
        tys.extend(enum_variant_field_types(variant)?);
    }
    alloc_enabled_impl_tokens(&item.ident, tys)
}

fn alloc_enabled_impl_tokens(
    ident: &syn::Ident,
    tys: Vec<&syn::Type>,
) -> syn::Result<TokenStream> {
    if tys.is_empty() {
        return Ok(quote! {
            impl ::sdk::internal::ContractAbiAllocEnabled for #ident {}
        });
    }

    Ok(quote! {
        impl ::sdk::internal::ContractAbiAllocEnabled for #ident
        where
            #(#tys: ::sdk::internal::ContractAbiAllocEnabled),*
        {}
    })
}

fn fields_type_defs_tokens(
    fields: &syn::Fields,
) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                codegen::contract::contract_type_typedef_tokens(&field.ty)
            })
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .map(|field| {
                codegen::contract::contract_type_typedef_tokens(&field.ty)
            })
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn field_types(fields: &syn::Fields) -> syn::Result<Vec<&syn::Type>> {
    match fields {
        syn::Fields::Named(named) => {
            Ok(named.named.iter().map(|field| &field.ty).collect())
        }
        syn::Fields::Unnamed(unnamed) => {
            Ok(unnamed.unnamed.iter().map(|field| &field.ty).collect())
        }
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn enum_variant_type_defs_tokens(
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    match &variant.fields {
        syn::Fields::Unit => {
            Ok(quote! { ::sdk::__contract_spec::ContractTypeDefsView::Empty })
        }
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            codegen::contract::contract_type_typedef_tokens(
                &fields.unnamed.first().unwrap().ty,
            )
        }
        syn::Fields::Named(fields) if fields.named.len() == 1 => {
            codegen::contract::contract_type_typedef_tokens(
                &fields.named.first().unwrap().ty,
            )
        }
        _ => Err(syn::Error::new_spanned(
            variant,
            "#[contract_error] variants with multiple payload fields are not yet supported in contract specs",
        )),
    }
}

fn enum_variant_field_types(
    variant: &syn::Variant,
) -> syn::Result<Vec<&syn::Type>> {
    match &variant.fields {
        syn::Fields::Unit => Ok(vec![]),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            Ok(vec![&fields.unnamed.first().unwrap().ty])
        }
        syn::Fields::Named(fields) if fields.named.len() == 1 => {
            Ok(vec![&fields.named.first().unwrap().ty])
        }
        _ => Err(syn::Error::new_spanned(
            variant,
            "#[contract_error] variants with multiple payload fields are not yet supported in contract specs",
        )),
    }
}

fn struct_type_def_tokens(item: &syn::ItemStruct) -> syn::Result<TokenStream> {
    let ident = &item.ident;
    let fields = struct_field_tokens(&item.fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefView::Struct(
            ::sdk::__contract_spec::ContractStructDefView {
                name: stringify!(#ident),
                fields: &[#(#fields),*],
            }
        )
    })
}

fn enum_type_def_tokens(item: &syn::ItemEnum) -> syn::Result<TokenStream> {
    let ident = &item.ident;
    let discriminants = enum_discriminants(&item.variants)?;
    let is_plain_enum = item
        .variants
        .iter()
        .all(|variant| matches!(variant.fields, syn::Fields::Unit));

    if is_plain_enum {
        let variants = item
            .variants
            .iter()
            .zip(discriminants.iter())
            .map(|(variant, discriminant)| {
                let name = variant.ident.to_string();
                quote! {
                    ::sdk::__contract_spec::ContractEnumVariantView {
                        name: #name,
                        value: #discriminant,
                    }
                }
            })
            .collect::<Vec<_>>();

        return Ok(quote! {
            ::sdk::__contract_spec::ContractTypeDefView::Enum(
                ::sdk::__contract_spec::ContractEnumDefView {
                    name: stringify!(#ident),
                    variants: &[#(#variants),*],
                }
            )
        });
    }

    let cases = item
        .variants
        .iter()
        .zip(discriminants.iter())
        .map(|(variant, discriminant)| {
            let name = variant.ident.to_string();
            let case_type = enum_variant_case_type_tokens(variant)?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractUnionCaseView {
                    name: #name,
                    discriminant: #discriminant,
                    case_type: #case_type,
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefView::Union(
            ::sdk::__contract_spec::ContractUnionDefView {
                name: stringify!(#ident),
                cases: &[#(#cases),*],
            }
        )
    })
}

fn struct_field_tokens(fields: &syn::Fields) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let name = field.ident.as_ref().unwrap().to_string();
                let ty =
                    codegen::contract::contract_type_view_tokens(&field.ty)?;
                Ok(quote! {
                    ::sdk::__contract_spec::ContractFieldView {
                        name: #name,
                        ty: #ty,
                    }
                })
            })
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let name = format!("_{index}");
                let ty =
                    codegen::contract::contract_type_view_tokens(&field.ty)?;
                Ok(quote! {
                    ::sdk::__contract_spec::ContractFieldView {
                        name: #name,
                        ty: #ty,
                    }
                })
            })
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn enum_variant_case_type_tokens(
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    match &variant.fields {
        syn::Fields::Unit => Ok(quote! { None }),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = codegen::contract::contract_type_view_tokens(
                &fields.unnamed.first().unwrap().ty,
            )?;
            Ok(quote! { Some(#ty) })
        }
        syn::Fields::Named(fields) if fields.named.len() == 1 => {
            let ty = codegen::contract::contract_type_view_tokens(
                &fields.named.first().unwrap().ty,
            )?;
            Ok(quote! { Some(#ty) })
        }
        _ => Err(syn::Error::new_spanned(
            variant,
            "#[contract_error] variants with multiple payload fields are not yet supported in contract specs",
        )),
    }
}

fn enum_discriminants(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<i32>> {
    let mut next = 0_i32;
    let mut values = Vec::with_capacity(variants.len());

    for variant in variants {
        let value = match &variant.discriminant {
            Some((_, expr)) => parse_i32_expr(expr)?,
            None => next,
        };
        values.push(value);
        next = value.checked_add(1).ok_or_else(|| {
            syn::Error::new_spanned(
                variant,
                "enum discriminant overflow in #[contract_error]",
            )
        })?;
    }

    Ok(values)
}

fn parse_i32_expr(expr: &syn::Expr) -> syn::Result<i32> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(value),
            ..
        }) => value.base10_parse::<i32>(),
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => parse_i32_expr(expr).map(|value| -value),
        _ => Err(syn::Error::new_spanned(
            expr,
            "#[contract_error] discriminants must be integer literals for contract specs",
        )),
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
