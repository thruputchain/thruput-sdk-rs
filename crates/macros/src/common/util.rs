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
            panic!("Can't handle non-ident arg pattern");
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
