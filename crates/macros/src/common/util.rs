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
    if thruput_contract_util::semver::try_major_version_from_string(version)
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

/// Validate a user-supplied `name = "..."` before it becomes the declared
/// segment of a spec id. `span` should be the span of the attribute value so
/// the error points at it.
///
/// `::` is the id's path separator, and the path is derived from where the
/// item is declared, not chosen by its author. A name carrying one would spell
/// path segments of its own, and an id joined for display could no longer
/// tell `name = "a::B"` from a `B` declared in a module `a`.
pub fn validate_spec_name(
    span: proc_macro2::Span,
    name: &str,
) -> Result<(), Error> {
    if name.contains("::") {
        return Err(Error::new(
            span,
            format!(
                "name = {name:?} must not contain `::` (it replaces only \
                 the final segment of the spec id; the module path comes \
                 from where the type is declared)"
            ),
        ));
    }
    Ok(())
}

/// Validate an explicit package label used only in an on-wire hash preimage.
pub fn validate_wire_crate_name(
    span: proc_macro2::Span,
    name: &str,
) -> syn::Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(syn::Error::new(
            span,
            "wire_crate must be a non-empty package label containing only ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_name_without_a_path_separator_is_accepted() {
        validate_spec_name(proc_macro2::Span::call_site(), "SpecPayload")
            .unwrap();
    }

    /// Wherever it sits, a `::` would read as a path separator.
    #[test]
    fn a_spec_name_carrying_a_path_separator_anywhere_is_rejected() {
        for name in ["spelled::Out", "::Out", "Out::", "::", "a::b::C"] {
            let err = validate_spec_name(proc_macro2::Span::call_site(), name)
                .unwrap_err();
            assert!(
                err.to_string().contains("must not contain `::`"),
                "{name:?}: {err}"
            );
        }
    }
}
