use groundhog_codegen as codegen;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;

/// Expand `#[derive(ContractTypeSpec)]` into `impl ContractTypeSpec` plus a
/// raw typedef wasm fragment.
pub fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ContractTypeSpec cannot be derived for generic types",
        ));
    }

    let (type_def, type_defs) = match &input.data {
        syn::Data::Struct(data) => (
            struct_type_def_tokens(ident, &data.fields)?,
            struct_type_defs_tokens(&data.fields)?,
        ),
        syn::Data::Enum(data) => (
            enum_type_def_tokens(ident, &data.variants)?,
            enum_type_defs_tokens(&data.variants)?,
        ),
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "ContractTypeSpec cannot be derived for unions",
            ));
        }
    };

    let raw_typedef = emit_raw_typedef_fragment(ident);

    Ok(quote! {
        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #raw_typedef
    })
}

fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    codegen::spec_reg::emit_raw_typedef_fragment_stream_tokens_with_cfg(
        quote! { target_family = "wasm" },
        quote! { <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEFS },
    )
}

fn struct_type_def_tokens(
    _ident: &syn::Ident,
    fields: &syn::Fields,
) -> syn::Result<TokenStream> {
    let field_tokens = struct_field_tokens(fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefView::Struct(
            ::sdk::__contract_spec::ContractStructDefView {
                name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
                fields: &[#(#field_tokens),*],
            }
        )
    })
}

fn struct_type_defs_tokens(fields: &syn::Fields) -> syn::Result<TokenStream> {
    let nested = fields_type_defs_tokens(fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            ::sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
            #(#nested),*
        ])
    })
}

fn enum_type_def_tokens(
    _ident: &syn::Ident,
    variants: &Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<TokenStream> {
    let discriminants = enum_discriminant_tokens(variants)?;
    let is_plain_enum = variants
        .iter()
        .all(|variant| matches!(variant.fields, syn::Fields::Unit));

    if is_plain_enum {
        let variant_tokens = variants
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
                    name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
                    variants: &[#(#variant_tokens),*],
                }
            )
        });
    }

    let cases = variants
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
                name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
                cases: &[#(#cases),*],
            }
        )
    })
}

fn enum_type_defs_tokens(
    variants: &Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<TokenStream> {
    let nested = variants
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

fn enum_discriminant_tokens(
    variants: &Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<TokenStream>> {
    variants
        .iter()
        .map(|variant| {
            let Some((_, expr)) = &variant.discriminant else {
                return Err(syn::Error::new_spanned(
                    variant,
                    "enum variants must use explicit discriminants for contract specs",
                ));
            };
            Ok(quote! { #expr })
        })
        .collect()
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
        syn::Fields::Unit => Ok(quote! {
            ::sdk::__contract_spec::ContractUnionCaseTypeView::Void
        }),
        syn::Fields::Unnamed(fields) => {
            let tys = fields
                .unnamed
                .iter()
                .map(|f| codegen::contract::contract_type_view_tokens(&f.ty))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractUnionCaseTypeView::Tuple(
                    &[#(#tys),*]
                )
            })
        }
        syn::Fields::Named(fields) => {
            let field_tokens = fields
                .named
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().unwrap().to_string();
                    let ty = codegen::contract::contract_type_view_tokens(
                        &field.ty,
                    )?;
                    Ok(quote! {
                        ::sdk::__contract_spec::ContractFieldView {
                            name: #name,
                            ty: #ty,
                        }
                    })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractUnionCaseTypeView::Struct(
                    &[#(#field_tokens),*]
                )
            })
        }
    }
}

fn enum_variant_type_defs_tokens(
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    let nested = fields_type_defs_tokens(&variant.fields)?;
    if nested.is_empty() {
        return Ok(
            quote! { ::sdk::__contract_spec::ContractTypeDefsView::Empty },
        );
    }
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            #(#nested),*
        ])
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

#[cfg(test)]
mod tests {
    use super::enum_type_def_tokens;

    #[test]
    fn enum_type_def_tokens_accepts_explicit_i32_min_discriminant() {
        let ident: syn::Ident = syn::parse_quote!(Error);
        let variants: syn::punctuated::Punctuated<
            syn::Variant,
            syn::Token![,],
        > = syn::parse_quote! {
            Min = -2147483647 - 1,
        };

        let tokens =
            enum_type_def_tokens(&ident, &variants).unwrap().to_string();
        assert!(tokens.contains("- 2147483647 - 1"));
    }
}
