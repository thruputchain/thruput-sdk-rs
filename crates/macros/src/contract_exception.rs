use darling::FromMeta;
use darling::util::SpannedValue;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ContractExceptionArgs {
    pub version: Option<SpannedValue<String>>,
    pub name: Option<String>,
}

/// Expand `#[contract_exception]` or `#[contract_exception(version = "...")]`
/// into the standard derive set + `#[repr(i32)]` (enums only) + `impl Exn`.
///
/// Supported on enums and structs. All user-supplied attributes (including
/// `#[error("...")]` from thiserror) are passed through unchanged.
pub fn expand(args: ContractExceptionArgs, item: Item) -> TokenStream {
    if let Some(version) = &args.version
        && let Err(err) = crate::common::util::validate_version_string(
            version.span(),
            version,
        )
    {
        return err.into_compile_error();
    }

    // The generated `impl Exn`/`ContractTypeName` use the bare ident, so a
    // generic exception type would produce cascading missing-generics
    // errors inside generated code.
    let generics = match &item {
        Item::Enum(e) => Some(&e.generics),
        Item::Struct(s) => Some(&s.generics),
        _ => None,
    };
    if let Some(generics) = generics
        && let Err(err) = crate::common::util::validate_no_generics(
            generics,
            false,
            "#[contract_exception] types",
        )
    {
        return err.into_compile_error();
    }

    match item {
        Item::Enum(e) => expand_enum(args, e),
        Item::Struct(s) => expand_struct(args, s),
        other => syn::Error::new_spanned(
            other,
            "#[contract_exception] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

fn expand_contract_type_name(
    ident: &syn::Ident,
    args: &ContractExceptionArgs,
) -> TokenStream {
    crate::contract_type_name_impl(
        &crate::common::contract::sdk_crate_path(),
        ident,
        args.name.as_deref(),
        // The same version the selector is derived from, so the two identities
        // cannot drift: a pinned exception keeps both across a crate release.
        args.version.as_ref().map(|version| version.as_str()),
    )
}

fn expand_enum(
    args: ContractExceptionArgs,
    item: syn::ItemEnum,
) -> TokenStream {
    if let Err(err) = crate::check_enum_discriminants(&item.variants) {
        return err.into_compile_error();
    }
    let ident = &item.ident;
    let ident_string = ident.to_string();
    let type_name = args.name.as_deref().unwrap_or(&ident_string);
    let selector = compute_selector(&args, type_name);
    let name_impl = expand_contract_type_name(ident, &args);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #name_impl
    }
}

fn expand_struct(
    args: ContractExceptionArgs,
    item: syn::ItemStruct,
) -> TokenStream {
    let ident = &item.ident;
    let ident_string = ident.to_string();
    let type_name = args.name.as_deref().unwrap_or(&ident_string);
    let selector = compute_selector(&args, type_name);
    let name_impl = expand_contract_type_name(ident, &args);

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #name_impl
    }
}

fn compute_selector(args: &ContractExceptionArgs, type_name: &str) -> u64 {
    let version = args
        .version
        .as_ref()
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION")
                .unwrap_or_else(|_| "0.0.0".to_string())
        });
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());
    groundhog_contract_util::exception_selector(
        &version,
        &crate_name,
        type_name,
    )
}
