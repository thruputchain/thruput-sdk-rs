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
        }

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
        }

        #raw_typedef
    }
}

fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    quote! {
        #[cfg(target_family = "wasm")]
        const _: () = {
            const FRAGMENT: ::sdk::__contract_spec::SpecFragmentView<'static> =
                ::sdk::__contract_spec::SpecFragmentView::TypeDef(
                    <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
                );
            const RAW_LEN: usize = FRAGMENT.const_txs_size();
            const RAW_FRAGMENT: [u8; RAW_LEN] = {
                let mut w = ::sdk::__xdrs::txs::ConstWriter::<RAW_LEN>::new();
                FRAGMENT.const_write_txs(&mut w);
                w.into_bytes()
            };

            #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
            pub static RAW_TYPE_DEF_SPEC: [u8; RAW_LEN] = RAW_FRAGMENT;
        };
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
