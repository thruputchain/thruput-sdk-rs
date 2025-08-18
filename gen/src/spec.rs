use crate::contract::ContractSrc;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;
use crate::util::*;

pub fn contract_repr(contract: &impl ContractSrc) -> Result<TokenStream> {
    let iface_name = contract.interface_name().unwrap_or_else(|| String::new());
    let iface_vers = contract.interface_vers().unwrap_or_else(|| "0.0.0".to_string());

    let method_reprs: Vec<_> =
        contract.methods_sorted()
            .into_iter().enumerate().map(|(index, method)|
                method_repr(index, method, contract)).collect::<Result<Vec<TokenStream>>>()?;

    Ok(quote! {
        {
            use ::xdrs::xdr_repr::*;
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

fn method_repr(_index: usize, method: &syn::Signature, contract: &impl ContractSrc) -> Result<TokenStream> {
    let name = method.ident.to_string();
    let args = method.inputs.iter().map(arg_repr)
        .collect::<Result<Vec<Option<TokenStream>>>>()?
        .into_iter()
        .flatten();
    let ret = ret_repr(&method.output);
    let err = if let Some(err) = contract.err_type() {
        quote! {
            Some(<#err as HasTypeSpec>::type_spec())
        }
    } else {
        quote! { None }
    };

    Ok(quote! {
        Method {
            name: #name.to_owned(),
            args: vec![ #(#args),* ],
            ret: #ret,
            err: #err
        }
    })
}

fn ret_repr(ret_ty: &syn::ReturnType) -> TokenStream {
    match ret_ty {
        syn::ReturnType::Default => quote! { None },
        syn::ReturnType::Type(_, ty) => quote! {
            Some(<#ty as HasTypeSpec>::type_spec())
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
        _ => Err(error(pt, "couldn't interpret method arg"))
    }
}
