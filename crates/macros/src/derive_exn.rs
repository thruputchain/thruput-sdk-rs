use darling::FromMeta;
use darling::ast::NestedMeta;
use darling::util::SpannedValue;
use proc_macro::TokenStream;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ExnArgs {
    pub version: Option<SpannedValue<String>>,
    pub wire_crate: Option<SpannedValue<String>>,
}

/// Parse `#[exn(...)]` or `#[contract_exception(...)]` attribute args.
pub fn parse_args(attr_args: TokenStream) -> Result<ExnArgs, TokenStream> {
    if attr_args.is_empty() {
        return Ok(ExnArgs::default());
    }
    let meta_list =
        NestedMeta::parse_meta_list(attr_args.into()).map_err(|e| {
            TokenStream::from(darling::Error::from(e).write_errors())
        })?;
    ExnArgs::from_list(&meta_list)
        .map_err(|e| TokenStream::from(e.write_errors()))
}

/// `#[exn]` or `#[exn(version = "1.0.0")]` attribute macro.
///
/// Emits `impl Exn for Type { const SELECTOR: u64 = ...; }`.
/// Version defaults to CARGO_PKG_VERSION if not specified, and the package
/// label in the selector hash to CARGO_PKG_NAME unless `wire_crate` pins it.
pub fn expand(args: ExnArgs, item: Item) -> TokenStream {
    if let Some(version) = &args.version
        && let Err(err) = crate::common::util::validate_version_string(
            version.span(),
            version,
        )
    {
        return err.into_compile_error().into();
    }
    if let Some(wire_crate) = &args.wire_crate
        && let Err(err) = crate::common::util::validate_wire_crate_name(
            wire_crate.span(),
            wire_crate,
        )
    {
        return err.into_compile_error().into();
    }

    let (ident, generics) = match &item {
        Item::Enum(e) => (&e.ident, &e.generics),
        Item::Struct(s) => (&s.ident, &s.generics),
        _ => {
            return syn::Error::new_spanned(
                item,
                "#[exn] is only supported on enums and structs",
            )
            .into_compile_error()
            .into();
        }
    };

    let type_name = ident.to_string();

    let version = args
        .version
        .as_ref()
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION")
                .unwrap_or_else(|_| "0.0.0".to_string())
        });
    let crate_name = args
        .wire_crate
        .as_ref()
        .map(|name| name.to_string())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_NAME")
                .unwrap_or_else(|_| "_unknown_".to_string())
        });

    let selector = thruput_contract_util::exception_selector(
        &version,
        &crate_name,
        &type_name,
    );

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let output = quote::quote! {
        #item

        impl #impl_generics ::thruput_sdk::exn::Exn for #ident #ty_generics #where_clause {
            const SELECTOR: u64 = #selector;
        }
    };

    output.into()
}
