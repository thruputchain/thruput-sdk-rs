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

    let Args { export, krate } = parse_args(input)?;

    let (type_def, anchor_refs) = match &input.data {
        syn::Data::Struct(data) => (
            struct_type_def_tokens(&krate, ident, &data.fields)?,
            field_anchor_tokens(&krate, &data.fields)?,
        ),
        syn::Data::Enum(data) => (
            enum_type_def_tokens(&krate, ident, &data.variants)?,
            enum_anchor_tokens(&krate, &data.variants)?,
        ),
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "ContractTypeSpec cannot be derived for unions",
            ));
        }
    };

    let raw_typedef = emit_raw_typedef_fragment(&krate, ident, export);
    let anchor_stmts = anchor_refs.iter().map(|anchor| {
        quote! {
            acc = #krate::__contract_spec::mix_anchor_fn(acc, #anchor);
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
            #[cfg(target_arch = "riscv32")]
            {
                let mut acc = 0usize;
                #(#anchor_stmts)*
                acc
            }
            #[cfg(not(target_arch = "riscv32"))]
            {
                0usize
            }
        }

        impl #krate::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: #krate::__contract_spec::ContractTypeDefView<'static> = #type_def;
        }

        impl #krate::__contract_spec::ContractTypeRefSpec for #ident {
            const INFO: #krate::__contract_spec::ContractTypeInfo<'static> = #krate::__contract_spec::ContractTypeInfo {
                ty: #krate::__contract_spec::ContractTypeView::Named(
                    <Self as #krate::__contract_spec::ContractTypeName>::NAME
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

struct Args {
    export: bool,
    /// Crate path the emitted spec traits resolve through. Defaults to
    /// `::sdk`; `#[contract_type_spec(crate = "...")]` overrides it so a crate
    /// that `sdk` depends on can derive without a dependency cycle. Everything
    /// reached through it is an `sdk` re-export of a crate below `sdk`, so the
    /// override only has to supply `__contract_spec` and `__xdrs`.
    krate: syn::Path,
}

fn parse_args(input: &syn::DeriveInput) -> syn::Result<Args> {
    use darling::FromMeta;

    #[derive(Debug, Default, FromMeta)]
    struct ContractTypeSpecArgs {
        #[darling(default)]
        export: bool,
        // `crate` is a keyword, so the field can't share its name.
        #[darling(default, rename = "crate")]
        krate: Option<syn::Path>,
    }

    let mut export = false;
    let mut krate: Option<syn::Path> = None;

    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("contract_type_spec"))
    {
        let args = ContractTypeSpecArgs::from_meta(&attr.meta)
            .map_err(|e| syn::Error::new_spanned(attr, e.to_string()))?;

        export |= args.export;

        if let Some(path) = args.krate {
            if krate.as_ref().is_some_and(|existing| *existing != path) {
                return Err(syn::Error::new_spanned(
                    attr,
                    "conflicting `crate` values for `contract_type_spec`",
                ));
            }
            krate = Some(path);
        }
    }

    Ok(Args {
        export,
        krate: krate.unwrap_or_else(common::contract::sdk_crate_path),
    })
}

fn emit_raw_typedef_fragment(
    krate: &syn::Path,
    ident: &syn::Ident,
    export: bool,
) -> TokenStream {
    let fragment_ident = format_ident!("__groundhog_spec_fragment_{ident}");
    let raw_len_ident = format_ident!("__groundhog_spec_raw_len_{ident}");
    let raw_fragment_ident =
        format_ident!("__groundhog_spec_raw_typedef_fragment_{ident}");

    let fragment_variant = if export {
        quote! { #krate::__contract_spec::SpecFragmentView::ExportedTypeDef(
            <#ident as #krate::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    } else {
        quote! { #krate::__contract_spec::SpecFragmentView::TypeDef(
            <#ident as #krate::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    };

    // `link_section` requires a string literal. Keep this in sync with
    // `groundhog_spec_tool::RAW_SECTION_NAME`; the spec-tool tests guard drift.
    // On RV32 this is an ALLOC data section, retained by `#[used]`, and the
    // groundhog-rpc lowerer excludes it from VM memory before artifact packing.
    quote! {
        #[doc(hidden)]
        pub const #fragment_ident: #krate::__contract_spec::SpecFragmentView<'static> =
            #fragment_variant;

        #[doc(hidden)]
        const #raw_len_ident: usize = #fragment_ident.const_txs_size();

        #[cfg(target_arch = "riscv32")]
        #[doc(hidden)]
        #[unsafe(link_section = "groundhogspecrawv3")]
        // `#[used]` keeps the section live through ELF linking so the CLI
        // can extract it.
        #[used]
        static #raw_fragment_ident: [u8; #raw_len_ident] = {
            let mut w =
                #krate::__xdrs::txs::ConstWriter::<#raw_len_ident>::new();
            #fragment_ident.const_write_txs(&mut w);
            w.into_bytes()
        };
    }
}

fn struct_type_def_tokens(
    krate: &syn::Path,
    _ident: &syn::Ident,
    fields: &syn::Fields,
) -> syn::Result<TokenStream> {
    let field_tokens = struct_field_tokens(krate, fields)?;
    Ok(quote! {
        #krate::__contract_spec::ContractTypeDefView::Struct(
            #krate::__contract_spec::ContractStructDefView {
                name: <Self as #krate::__contract_spec::ContractTypeName>::NAME,
                fields: &[#(#field_tokens),*],
            }
        )
    })
}

fn enum_type_def_tokens(
    krate: &syn::Path,
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
                    #krate::__contract_spec::ContractEnumVariantView {
                        name: #name,
                        value: #discriminant,
                    }
                }
            })
            .collect::<Vec<_>>();

        return Ok(quote! {
            #krate::__contract_spec::ContractTypeDefView::Enum(
                #krate::__contract_spec::ContractEnumDefView {
                    name: <Self as #krate::__contract_spec::ContractTypeName>::NAME,
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
            let case_type = enum_variant_case_type_tokens(krate, variant)?;
            Ok(quote! {
                #krate::__contract_spec::ContractUnionCaseView {
                    name: #name,
                    discriminant: #discriminant,
                    case_type: #case_type,
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        #krate::__contract_spec::ContractTypeDefView::Union(
            #krate::__contract_spec::ContractUnionDefView {
                name: <Self as #krate::__contract_spec::ContractTypeName>::NAME,
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

fn struct_field_tokens(
    krate: &syn::Path,
    fields: &syn::Fields,
) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let name = field.ident.as_ref().unwrap().to_string();
                let ty = common::contract::contract_type_view_tokens_in(
                    krate, &field.ty,
                )?;
                Ok(quote! {
                    #krate::__contract_spec::ContractFieldView {
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
                let ty = common::contract::contract_type_view_tokens_in(
                    krate, &field.ty,
                )?;
                Ok(quote! {
                    #krate::__contract_spec::ContractFieldView {
                        name: #name,
                        ty: #ty,
                    }
                })
            })
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn field_anchor_tokens(
    krate: &syn::Path,
    fields: &syn::Fields,
) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| field_anchor_token(krate, field))
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .map(|field| field_anchor_token(krate, field))
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn field_anchor_token(
    krate: &syn::Path,
    field: &syn::Field,
) -> syn::Result<TokenStream> {
    let ty = &field.ty;
    Ok(quote! {
        <#ty as #krate::__contract_spec::ContractTypeRefSpec>::__GROUNDHOG_SPEC_ANCHOR_ROOT
    })
}

fn enum_variant_case_type_tokens(
    krate: &syn::Path,
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    match &variant.fields {
        syn::Fields::Unit => Ok(quote! {
            #krate::__contract_spec::ContractUnionCaseTypeView::Void
        }),
        syn::Fields::Unnamed(fields) => {
            let tys = fields
                .unnamed
                .iter()
                .map(|f| {
                    common::contract::contract_type_view_tokens_in(
                        krate, &f.ty,
                    )
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                #krate::__contract_spec::ContractUnionCaseTypeView::Tuple(
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
                    let ty = common::contract::contract_type_view_tokens_in(
                        krate, &field.ty,
                    )?;
                    Ok(quote! {
                        #krate::__contract_spec::ContractFieldView {
                            name: #name,
                            ty: #ty,
                        }
                    })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! {
                #krate::__contract_spec::ContractUnionCaseTypeView::Struct(
                    &[#(#field_tokens),*]
                )
            })
        }
    }
}

fn enum_anchor_tokens(
    krate: &syn::Path,
    variants: &Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<TokenStream>> {
    let mut anchors = Vec::new();
    for variant in variants {
        anchors.extend(field_anchor_tokens(krate, &variant.fields)?);
    }
    Ok(anchors)
}

#[cfg(test)]
mod tests {
    use super::{enum_type_def_tokens, expand};
    use crate::common::contract::sdk_crate_path;

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
            enum_type_def_tokens(&sdk_crate_path(), &ident, &variants)
                .unwrap()
                .to_string();
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
        // The raw fragment static is rv32-gated and carries both attributes
        // the CLI's extraction depends on: the section name, and `#[used]` to
        // survive linking. Gated by the `#[cfg]` on the item, so the
        // attributes themselves are unconditional.
        assert!(
            tokens.contains(
                "# [cfg (target_arch = \"riscv32\")] # [doc (hidden)] \
                 # [unsafe (link_section = \"groundhogspecrawv3\")] # [used] \
                 static __groundhog_spec_raw_typedef_fragment_Envelope"
            ),
            "{tokens}"
        );
        assert!(!tokens.contains("mix_anchor_ptr"), "{tokens}");
    }

    #[test]
    fn crate_override_replaces_every_sdk_path() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[contract_type_spec(crate = "::groundhog_u256::__sdk_paths")]
            struct U256([u32; 8]);
        };

        let tokens = expand(&input).unwrap().to_string();

        // The point of the option: a crate below `sdk` in the dependency graph
        // cannot name `sdk` at all, so a single leaked path breaks the build.
        assert!(!tokens.contains("sdk ::"), "{tokens}");
        assert!(
            tokens.contains(
                ":: groundhog_u256 :: __sdk_paths :: __contract_spec :: ContractTypeSpec"
            ),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                ":: groundhog_u256 :: __sdk_paths :: __xdrs :: txs :: ConstWriter"
            ),
            "{tokens}"
        );
    }

    #[test]
    fn crate_override_applies_to_field_types_and_anchors() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[contract_type_spec(crate = "::shim")]
            struct Envelope {
                payload: Option<Vec<Payload>>,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        // Field types route through `common::contract`, which is shared with
        // the out-of-scope macros; easy to leave behind on `::sdk`.
        assert!(
            tokens.contains(":: shim :: __contract_spec :: type_ref"),
            "{tokens}"
        );
        assert!(
            tokens.contains(
                "Option < Vec < Payload > > as :: shim :: __contract_spec :: ContractTypeRefSpec"
            ),
            "{tokens}"
        );
        assert!(!tokens.contains("sdk ::"), "{tokens}");
    }

    #[test]
    fn crate_override_composes_with_export() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[contract_type_spec(export, crate = "::shim")]
            struct Exported {
                value: u32,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains(
                ":: shim :: __contract_spec :: SpecFragmentView :: ExportedTypeDef"
            ),
            "{tokens}"
        );
        assert!(!tokens.contains("sdk ::"), "{tokens}");
    }

    #[test]
    fn export_still_ors_across_repeated_attributes() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[contract_type_spec(crate = "::shim")]
            #[contract_type_spec(export)]
            struct Exported {
                value: u32,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains(
                ":: shim :: __contract_spec :: SpecFragmentView :: ExportedTypeDef"
            ),
            "{tokens}"
        );
    }

    #[test]
    fn conflicting_crate_values_are_rejected() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[contract_type_spec(crate = "::a")]
            #[contract_type_spec(crate = "::b")]
            struct Confused {
                value: u32,
            }
        };

        let err = expand(&input).unwrap_err().to_string();

        assert!(err.contains("conflicting `crate`"), "{err}");
    }

    #[test]
    fn default_crate_path_is_unchanged() {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Plain {
                value: u32,
            }
        };

        let tokens = expand(&input).unwrap().to_string();

        assert!(
            tokens.contains(":: sdk :: __contract_spec :: ContractTypeSpec"),
            "{tokens}"
        );
        assert!(
            tokens.contains(":: sdk :: __xdrs :: txs :: ConstWriter"),
            "{tokens}"
        );
        assert!(
            tokens.contains(":: sdk :: __contract_spec :: type_ref"),
            "{tokens}"
        );
    }
}
