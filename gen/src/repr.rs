use crate::contract::ContractSrc;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;
use crate::util::error;

pub fn contract_repr(contract: &impl ContractSrc) -> Result<TokenStream> {
    let method_reprs: Vec<_> =
        contract.methods_sorted()
            .into_iter().enumerate().map(|(index, method)|
                method_repr(index, method, contract)).collect::<Result<Vec<TokenStream>>>()?;

    Ok(quote! {
        {
            use ::sdk::interface::*;
            use ::xdr_repr::xdr as xdr;
            use ::xdr_repr::ty::*;
            use ::std::vec;

            Contract {
                methods: vec![ #(#method_reprs),* ]
            }
        }
    })
}

fn method_repr(_index: usize, method: &syn::Signature, _contract: &impl ContractSrc) -> Result<TokenStream> {
    let name = method.ident.to_string();
    let args = method.inputs.iter().map(arg_repr)
        .collect::<Result<Vec<Option<TokenStream>>>>()?
        .into_iter()
        .flatten();

    Ok(quote! {
        Method {
            name: #name.to_owned(),
            args: vec![ #(#args),* ]
        }
    })
}

fn arg_repr(fn_arg: &syn::FnArg) -> Result<Option<TokenStream>> {
    if let syn::FnArg::Typed(pat_type) = fn_arg {
        let ident = pat_type_ident(pat_type)?;
        let ty = pat_type.ty.as_ref();
        let name = ident.to_string();

        Ok(Some(quote! {
            Arg {
                name: #name.to_owned(),
                decl: <#ty as Declare>::declare(),
            }
        }))
    } else {
        Ok(None)
    }
}

fn pat_type_ident(pt: &syn::PatType) -> Result<&syn::Ident> {
    match pt.pat.as_ref() {
        syn::Pat::Ident(pat_ident) => Ok(&pat_ident.ident),
        _ => Err(error(pt, "couldn't interpret method arg"))
    }
}
