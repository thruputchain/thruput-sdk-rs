use super::common;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;

/// Expand `#[derive(ContractTypeSpec)]` into `impl ContractTypeSpec` plus a
/// raw typedef fragment for link-section based spec extraction.
pub fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ContractTypeSpec cannot be derived for generic types",
        ));
    }

    let (type_def, anchor_refs) = match &input.data {
        syn::Data::Struct(data) => (
            struct_type_def_tokens(ident, &data.fields)?,
            field_anchor_tokens(&data.fields)?,
        ),
        syn::Data::Enum(data) => (
            enum_type_def_tokens(ident, &data.variants)?,
            enum_anchor_tokens(&data.variants)?,
        ),
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "ContractTypeSpec cannot be derived for unions",
            ));
        }
    };

    let export = {
        use darling::FromMeta;
        #[derive(Debug, Default, FromMeta)]
        struct ContractTypeSpecArgs {
            #[darling(default)]
            export: bool,
        }
        input
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("contract_type_spec"))
            .try_fold(false, |acc, attr| {
                let meta = &attr.meta;
                let args = ContractTypeSpecArgs::from_meta(meta)?;
                Ok::<_, darling::Error>(acc || args.export)
            })
            .map_err(|e| syn::Error::new_spanned(ident, e.to_string()))?
    };
    let raw_typedef = emit_raw_typedef_fragment(ident, export);
    let anchor_stmts = anchor_refs.iter().map(|anchor| {
        quote! {
            acc = ::sdk::__contract_spec::mix_anchor_fn(acc, #anchor);
        }
    });
    let anchor_fn_ident = format_ident!("__groundhog_spec_anchor_fn_{ident}");

    Ok(quote! {
        #[doc(hidden)]
        #[inline(never)]
        #[cfg_attr(
            target_arch = "riscv32",
            unsafe(link_section = "groundhogspecanchortextv0")
        )]
        pub fn #anchor_fn_ident() -> usize {
            #[cfg(any(target_family = "wasm", target_arch = "riscv32"))]
            {
                let mut acc = 0usize;
                #(#anchor_stmts)*
                acc
            }
            #[cfg(not(any(target_family = "wasm", target_arch = "riscv32")))]
            {
                0usize
            }
        }

        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
        }

        impl ::sdk::__contract_spec::ContractTypeRefSpec for #ident {
            const INFO: ::sdk::__contract_spec::ContractTypeInfo<'static> = ::sdk::__contract_spec::ContractTypeInfo {
                ty: ::sdk::__contract_spec::ContractTypeView::Named(
                    <Self as ::sdk::__contract_spec::ContractTypeName>::NAME
                ),
            };

            const __GROUNDHOG_SPEC_ANCHOR_ROOT: fn() -> usize = #anchor_fn_ident;

            fn __groundhog_spec_anchor() -> usize {
                #anchor_fn_ident()
            }
        }

        #raw_typedef
    })
}

fn emit_raw_typedef_fragment(ident: &syn::Ident, export: bool) -> TokenStream {
    let fragment_ident = format_ident!("__groundhog_spec_fragment_{ident}");
    let raw_len_ident = format_ident!("__groundhog_spec_raw_len_{ident}");
    let raw_fragment_ident =
        format_ident!("__groundhog_spec_raw_typedef_fragment_{ident}");

    let fragment_variant = if export {
        quote! { ::sdk::__contract_spec::SpecFragmentView::ExportedTypeDef(
            <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    } else {
        quote! { ::sdk::__contract_spec::SpecFragmentView::TypeDef(
            <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    };

    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    // On RV32 this is an ALLOC data section, retained by `#[used]`, and the
    // groundhog-rpc lowerer excludes it from VM memory before artifact packing.
    quote! {
        #[doc(hidden)]
        pub const #fragment_ident: ::sdk::__contract_spec::SpecFragmentView<'static> =
            #fragment_variant;

        #[doc(hidden)]
        const #raw_len_ident: usize = #fragment_ident.const_txs_size();

        #[cfg(any(target_family = "wasm", target_arch = "riscv32"))]
        #[doc(hidden)]
        #[cfg_attr(
            any(target_family = "wasm", target_arch = "riscv32"),
            unsafe(link_section = "groundhogspecrawv1")
        )]
        // RV32 only: `#[used]` keeps the section live through ELF linking for CLI
        // extraction. On wasm the custom section is retained without it, and
        // `#[used]` there would force the fragment bytes into linear memory
        // (tripping the spec-tool leak guard).
        #[cfg_attr(target_arch = "riscv32", used)]
        static #raw_fragment_ident: [u8; #raw_len_ident] = {
            let mut w =
                ::sdk::__xdrs::txs::ConstWriter::<#raw_len_ident>::new();
            #fragment_ident.const_write_txs(&mut w);
            w.into_bytes()
        };
    }
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
                    common::contract::contract_type_view_tokens(&field.ty)?;
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
                    common::contract::contract_type_view_tokens(&field.ty)?;
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

fn field_anchor_tokens(fields: &syn::Fields) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => {
            named.named.iter().map(field_anchor_token).collect()
        }
        syn::Fields::Unnamed(unnamed) => {
            unnamed.unnamed.iter().map(field_anchor_token).collect()
        }
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn field_anchor_token(field: &syn::Field) -> syn::Result<TokenStream> {
    let ty = &field.ty;
    Ok(quote! {
        <#ty as ::sdk::__contract_spec::ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT
    })
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
                .map(|f| common::contract::contract_type_view_tokens(&f.ty))
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
                    let ty = common::contract::contract_type_view_tokens(
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

fn enum_anchor_tokens(
    variants: &Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<TokenStream>> {
    let mut anchors = Vec::new();
    for variant in variants {
        anchors.extend(field_anchor_tokens(&variant.fields)?);
    }
    Ok(anchors)
}

#[cfg(test)]
mod tests {
    use super::{enum_type_def_tokens, expand};

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

    #[test]
    fn expand_emits_anchor_for_nested_named_dependencies() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Envelope {
                payload: Option<Vec<Payload>>,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(tokens.contains("__groundhog_spec_anchor"), "{tokens}");
        assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
        assert!(tokens.contains("groundhogspecanchortextv0"), "{tokens}");
        assert!(
            tokens.contains("Option < Vec < Payload > > as :: sdk :: __contract_spec :: ContractTypeRefSpec"),
            "{tokens}"
        );
    }

    #[test]
    fn expand_emits_explicit_anchor_root_fn() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Envelope {
                payload: Payload,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains(
                "pub fn __groundhog_spec_anchor_fn_Envelope () -> usize"
            ),
            "{tokens}"
        );
        assert!(
            tokens.contains("const __GROUNDHOG_SPEC_ANCHOR_ROOT : fn () -> usize = __groundhog_spec_anchor_fn_Envelope"),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "used)] static __groundhog_spec_raw_typedef_fragment_Envelope"
            ),
            "{tokens}"
        );
        assert!(!tokens.contains("mix_anchor_ptr"), "{tokens}");
    }
}
