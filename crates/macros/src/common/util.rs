use core::cmp::Ordering;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Error;

pub fn cmp_sig_by_name(a: &syn::Signature, b: &syn::Signature) -> Ordering {
    cmp_ident(&a.ident, &b.ident)
}

pub fn cmp_ident(a: &syn::Ident, b: &syn::Ident) -> Ordering {
    a.to_string().cmp(&b.to_string())
}

pub fn is_receiver(fn_arg: &syn::FnArg) -> bool {
    matches!(fn_arg, syn::FnArg::Receiver(_))
}

pub fn arg_pat(fn_arg: &syn::FnArg) -> Option<&syn::Pat> {
    match fn_arg {
        syn::FnArg::Typed(pat_type) => Some(&pat_type.pat),
        syn::FnArg::Receiver(_) => None,
    }
}

pub fn method_args(
    sig: &syn::Signature,
) -> impl Iterator<Item = (&syn::Ident, &syn::Type)> {
    fn arg_from_pat_type(
        pat_type: &syn::PatType,
    ) -> (&syn::Ident, &syn::Type) {
        let ident = if let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
            &pat_ident.ident
        } else {
            // validate_method_arg_patterns rejects these with a spanned
            // error before any codegen path reaches here.
            panic!(
                "internal error: non-ident arg pattern should have been \
                 rejected by validate_method_arg_patterns"
            );
        };
        let ty = pat_type.ty.as_ref();
        (ident, ty)
    }

    sig.inputs.iter().filter_map(|fn_arg| match fn_arg {
        syn::FnArg::Typed(pat_type) => Some(arg_from_pat_type(pat_type)),
        _ => None,
    })
}

pub fn method_arg_names(
    sig: &syn::Signature,
) -> impl Iterator<Item = &syn::Ident> {
    method_args(sig).map(|(ident, _ty)| ident)
}

pub fn method_arg_name_list(sig: &syn::Signature) -> TokenStream {
    let idents = method_arg_names(sig);
    quote! { #( #idents ),* }
}

// Produce an error referencing a particular code span
pub fn error<S, M>(spanned: &S, msg: M) -> Error
where
    S: syn::spanned::Spanned,
    M: AsRef<str>,
{
    Error::new(spanned.span(), msg.as_ref())
}

/// Reject generic parameters on `subject`, with a spanned error on the
/// first offending parameter. Generated code (clients, entrypoint wrappers,
/// spec consts, `Exn` impls) is non-generic, so generics would otherwise
/// surface as unresolved-type errors deep inside code the author never
/// wrote. `allow_lifetimes` is only for positions where lifetime params
/// are genuinely handled (`#[derive(Event)]` folds them to `'static`);
/// everywhere else even explicit lifetimes are rejected — the generated
/// companion items declare no lifetime parameters, so a spliced `'a` dies
/// with E0261 in code the author never wrote. Elided borrow lifetimes are
/// not generics params and are unaffected.
///
/// The single shared guard for every macro that splices user types into
/// non-generic output — new macros should call this rather than hand-roll
/// the check (hand-rolled copies are how `#[contract_exception]` was
/// originally missed).
pub fn validate_no_generics(
    generics: &syn::Generics,
    allow_lifetimes: bool,
    subject: &str,
) -> Result<(), Error> {
    for param in &generics.params {
        if allow_lifetimes && matches!(param, syn::GenericParam::Lifetime(_)) {
            continue;
        }
        let msg = if allow_lifetimes {
            format!(
                "generic parameters are not supported on {subject} \
                 (lifetimes are allowed)"
            )
        } else {
            format!(
                "generic parameters (including lifetimes) are not \
                 supported on {subject}"
            )
        };
        return Err(error(param, msg));
    }
    Ok(())
}

/// Validate a user-supplied `version = "..."` attribute value before it
/// reaches the panicking semver parser inside the selector/entrypoint
/// hash derivation. `span` should be the span of the attribute value so
/// the error points at it.
pub fn validate_version_string(
    span: proc_macro2::Span,
    version: &str,
) -> Result<(), Error> {
    if groundhog_contract_util::semver::try_major_version_from_string(version)
        .is_none()
    {
        return Err(Error::new(
            span,
            format!(
                "invalid version {version:?}: expected a full semver \
                 triplet like \"1.0.0\" (optionally with -PRERELEASE \
                 and +BUILD suffixes)"
            ),
        ));
    }
    Ok(())
}

/// Validate that a user-supplied attribute string is a legal Rust
/// identifier before it reaches a panicking `Ident::new`/`format_ident!`.
/// `span` should be the span of the attribute value so the error points
/// at it.
pub fn validate_ident_string(
    span: proc_macro2::Span,
    value: &str,
    attr: &str,
) -> Result<(), Error> {
    if syn::parse_str::<syn::Ident>(value).is_err() {
        return Err(Error::new(
            span,
            format!(
                "{attr} = {value:?} must be a valid Rust identifier \
                 (it is used to name generated items)"
            ),
        ));
    }
    Ok(())
}

/// Validate a user-supplied `name = "..."` on a spec type.
///
/// The name becomes the final segment of the type's spec id
/// (`CRATE@MAJOR::module::path::Name`), so it must be one segment. A value
/// carrying `::` would splice extra segments into the id and could forge
/// another type's — `name = "inner::Meta"` in module `m` claims the id of a
/// real `m::inner::Meta` — which is the collision the qualified id exists to
/// prevent. Requiring an identifier also keeps the name usable as a declared
/// type name in generated bindings.
pub fn validate_spec_type_name(
    span: proc_macro2::Span,
    name: &str,
) -> Result<(), Error> {
    if syn::parse_str::<syn::Ident>(name).is_err() {
        return Err(Error::new(
            span,
            format!(
                "name = {name:?} must be a single identifier: it becomes the \
                 final segment of this type's spec id, and a name carrying \
                 `::` would claim another type's id"
            ),
        ));
    }
    Ok(())
}
