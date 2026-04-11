use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ContractErrorArgs {
    pub version: Option<String>,
    pub name: Option<String>,
}

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the standard derive set + `#[repr(i32)]` (enums only) + `impl Exn`.
///
/// Supported on enums and structs. All user-supplied attributes (including
/// `#[error("...")]` from thiserror) are passed through unchanged.
pub fn expand(args: ContractErrorArgs, item: Item) -> TokenStream {
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

fn expand_contract_type_name(
    ident: &syn::Ident,
    args: &ContractErrorArgs,
) -> TokenStream {
    crate::contract_type_name_impl(ident, args.name.as_deref())
}

fn expand_enum(args: ContractErrorArgs, item: syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let alloc_marker_impl = match enum_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let name_impl = expand_contract_type_name(ident, &args);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl
        #name_impl
    }
}

fn expand_struct(
    args: ContractErrorArgs,
    item: syn::ItemStruct,
) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let alloc_marker_impl = match struct_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let name_impl = expand_contract_type_name(ident, &args);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl
        #name_impl
    }
}

fn struct_alloc_enabled_impl_tokens(
    item: &syn::ItemStruct,
) -> syn::Result<TokenStream> {
    alloc_enabled_impl_tokens(&item.ident, field_types(&item.fields)?)
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
    _ident: &syn::Ident,
    tys: Vec<&syn::Type>,
) -> syn::Result<TokenStream> {
    if tys.is_empty() {
        return Ok(quote! {
            //impl ::sdk::internal::ContractAbiAllocEnabled for #ident {}
        });
    }

    Ok(quote! {
       //impl ::sdk::internal::ContractAbiAllocEnabled for #ident
       // where
       //     #(#tys: ::sdk::internal::ContractAbiAllocEnabled),*
       // {}
    })
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

fn compute_selector(args: &ContractErrorArgs, type_name: &str) -> u64 {
    let version = args.version.clone().unwrap_or_else(|| {
        std::env::var("CARGO_PKG_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string())
    });
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());
    groundhog_contract_util::error_selector(&version, &crate_name, type_name)
}
