use crate::contract::{ContractSrc, Method};
use crate::util::*;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

pub fn contract_repr(contract: &impl ContractSrc) -> Result<TokenStream> {
    let iface_name = contract.interface_name().unwrap_or_default();
    let iface_vers = contract.defaulted_interface_vers();

    let method_reprs: Vec<_> = contract
        .methods_sorted()
        .into_iter()
        .enumerate()
        .map(|(index, method)| method_repr(index, &method, contract))
        .collect::<Result<Vec<TokenStream>>>()?;

    Ok(quote! {
        {
            use ::sdk::spec::*;
            use ::std::vec;
            use ::std::string::*;
            use ::std::format;

            Contract {
                interface_id: #iface_name.to_string(),
                interface_vers: #iface_vers.to_string(),
                methods: vec![ #(#method_reprs),* ]
            }
        }
    })
}

fn method_repr(
    _index: usize,
    method: &impl Method,
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    let name = method.sig().ident.to_string();
    let args = method
        .sig()
        .inputs
        .iter()
        .map(arg_repr)
        .collect::<Result<Vec<Option<TokenStream>>>>()?
        .into_iter()
        .flatten();
    let ret = ret_repr(&method.sig().output);
    let errs = contract.throws_list();
    let err = if errs.is_empty() {
        quote! { vec![] }
    } else {
        let err_specs = errs.iter().map(|e| {
            let ty = e.ty();
            quote! { <#ty as HasTypeSpec>::type_spec() }
        });
        quote! { vec![ #(#err_specs),* ] }
    };

    Ok(quote! {
        Method {
            name: #name.to_owned(),
            args: vec![ #(#args),* ],
            ret: #ret,
            err: #err,
        }
    })
}

fn ret_repr(ret_ty: &syn::ReturnType) -> TokenStream {
    match ret_ty {
        syn::ReturnType::Default => quote! { None },
        // TODO(geoff): I switched this to use a Declaration instead of a typespec;
        // the TypeSpec was dropping the inner type info from types like Vec.
        syn::ReturnType::Type(_, ty) => quote! {
            Some(<#ty as Declare>::declare(ident("")))
        },
    }
}

fn arg_repr(fn_arg: &syn::FnArg) -> Result<Option<TokenStream>> {
    if let syn::FnArg::Typed(pat_type) = fn_arg {
        let ident = pat_type_ident(pat_type)?;
        let ty = pat_type.ty.as_ref();
        let name = ident.to_string();

        Ok(Some(quote! {
            Arg {
                name: #name.to_owned(),
                decl: <#ty as Declare>::declare(ident(#name)),
            }
        }))
    } else {
        Ok(None)
    }
}

fn pat_type_ident(pt: &syn::PatType) -> Result<&syn::Ident> {
    match pt.pat.as_ref() {
        syn::Pat::Ident(pat_ident) => Ok(&pat_ident.ident),
        _ => Err(error(pt, "couldn't interpret method arg")),
    }
}
