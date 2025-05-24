use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::parse::{Error, Result};
use syn::spanned::Spanned;

pub fn contract_output(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    // Parse the attributes
    let config = match NestedMeta::parse_meta_list(attr_args.into()) {
        Ok(meta_list) => match sig::Config::from_list(&meta_list) {
            Ok(config) => config,
            Err(e) => return TokenStream::from(e.write_errors()),
        },
        Err(e) => {
            return TokenStream::from(darling::Error::from(e).write_errors());
        }
    };

    // Parse the item tokens into syntax defined in `syn`
    let item = {
        let item_tokens = item_tokens.clone();

        syn::parse_macro_input!(item_tokens as syn::Item)
    };

    // Generate the helper code
    let helper_tokens = match item_to_contract_helpers(&item, &config) {
        Ok(x) => x,
        Err(err) => return Error::into_compile_error(err).into(),
    };

    // Construct the output
    let mut output = TokenStream::new();

    // First output the original item
    output.extend(item_tokens);

    // Then add the helper items
    let helper_tokens: TokenStream = helper_tokens.into();
    output.extend(helper_tokens);

    output
}

fn item_to_contract_helpers(
    item: &syn::Item,
    config: &sig::Config,
) -> Result<TokenStream2> {
    match item {
        syn::Item::Trait(item) => {
            let sig = parse::contract_signature_from_trait(item)?;
            let client = gen::client_from_contract_signature(&sig, config)?;

            Ok(client)
        }
        syn::Item::Impl(item) => {
            let sig = parse::contract_signature_from_impl(item)?;
            let mut out = TokenStream2::new();

            let exports = gen::exports_from_contract_signature(&sig, config)?;
            out.extend(exports.clone());

            let test_registration =
                gen::test_registration_from_method_signature(&sig, config)?;
            out.extend(test_registration.clone());

            Ok(out)
        }
        syn::Item::Fn(item) => {
            let sig = parse::method_signature_from_sig(&item.sig);
            let mut out = TokenStream2::new();

            // TODO(geoff): My understanding is that this case is for functions that
            // are exported not as part of an interface.  What should their method ID be?
            let method_id =
                gen::get_method_num(&item.sig.ident.to_string(), "0.1.0", 0);
            out.extend(gen::export_from_method_signature(
                None, &sig, method_id,
            )?);

            Ok(out)
        }
        _ => err(
            item,
            format!("Can't generate contract helpers for item {:?}", item),
        ),
    }
}

// Produce an error referencing a particular code span
fn err<T, S, M>(spanned: &S, msg: M) -> Result<T>
where
    S: Spanned,
    M: AsRef<str>,
{
    Err(Error::new(spanned.span(), msg.as_ref()))
}

mod sig {
    //! Represent contract signatures

    use darling::FromMeta;
    use proc_macro2::Span;

    #[derive(Debug, FromMeta)]
    pub struct Config {
        pub exports: Option<String>,
        pub client: Option<String>,
        pub test_setup: Option<String>,
    }

    impl Config {
        pub fn exports_mod_ident(&self) -> syn::Ident {
            ident_or(&self.exports, "exports")
        }

        pub fn client_mod_ident(&self) -> syn::Ident {
            ident_or(&self.client, "client")
        }

        pub fn test_setup_mod_ident(&self) -> syn::Ident {
            ident_or(&self.test_setup, "test_setup")
        }
    }

    fn ident_or(maybe_name: &Option<String>, default: &str) -> syn::Ident {
        let span = Span::call_site();
        match maybe_name {
            Some(ref name_string) => {
                syn::Ident::new(name_string.as_ref(), span)
            }
            None => syn::Ident::new(default, span),
        }
    }

    #[derive(Debug)]
    pub struct Contract {
        #[allow(unused)]
        pub span: Span,
        pub ident: syn::Ident,
        #[allow(unused)]
        pub generics: syn::Generics,
        pub methods: Vec<syn::Signature>,
    }

    #[derive(Debug)]
    pub struct ContractImpl {
        #[allow(unused)]
        pub span: Span,
        pub trait_: Option<syn::Path>,
        pub trait_ident: syn::Ident,
        #[allow(unused)]
        pub generics: syn::Generics,
        pub self_ty: Box<syn::Type>,
        pub methods: Vec<Method>,
    }

    #[derive(Debug)]
    pub struct Method {
        #[allow(unused)]
        pub span: Span,
        pub ident: syn::Ident,
        pub receiver: Option<syn::Receiver>,
        pub args: Vec<Arg>,
    }

    #[derive(Debug)]
    pub struct Arg {
        pub ident: syn::Ident,
        pub ty: syn::Type,
    }
}

mod parse {
    //! Parse syntax to contract signature

    use super::sig;
    use syn::parse::Result;
    use syn::spanned::Spanned;

    pub fn contract_signature_from_trait(
        item: &syn::ItemTrait,
    ) -> Result<sig::Contract> {
        // Pull out the method signatures
        let methods = item
            .items
            .iter()
            .filter_map(|item| {
                if let syn::TraitItem::Fn(trait_fn) = item {
                    Some(trait_fn.sig.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(sig::Contract {
            span: item.span(),
            ident: item.ident.clone(),
            generics: item.generics.clone(),
            methods,
        })
    }

    pub fn contract_signature_from_impl(
        item: &syn::ItemImpl,
    ) -> Result<sig::ContractImpl> {
        // Get the contract trait being implemented, if any
        let trait_ = item.trait_.clone().map(|(_, path, _)| path);
        let trait_ident = trait_
            .as_ref()
            .map(|path| path.segments.last().unwrap().ident.clone())
            .unwrap();

        // Pull out the function items
        let methods = item
            .items
            .iter()
            .filter_map(|item| {
                if let syn::ImplItem::Fn(ref impl_fn) = item {
                    let is_pub = is_visible(&impl_fn.vis);

                    // Only include public functions if this is not a trait impl
                    if trait_.is_some() || is_pub {
                        Some(method_signature_from_sig(&impl_fn.sig))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        Ok(sig::ContractImpl {
            span: item.span(),
            trait_,
            trait_ident,
            generics: item.generics.clone(),
            self_ty: item.self_ty.clone(),
            methods,
        })
    }

    fn is_visible(vis: &syn::Visibility) -> bool {
        matches!(vis, syn::Visibility::Public(_))
    }

    pub fn method_signature_from_sig(sig: &syn::Signature) -> sig::Method {
        let span = sig.span();
        let ident = sig.ident.clone();
        let mut receiver = None;
        let args = sig
            .inputs
            .iter()
            .filter_map(|fn_arg| match fn_arg {
                syn::FnArg::Typed(pat_type) => arg_from_pat_type(pat_type),
                syn::FnArg::Receiver(rcvr) => {
                    receiver = Some(rcvr.clone());
                    None
                }
            })
            .collect();
        sig::Method {
            span,
            ident,
            receiver,
            args,
        }
    }

    fn arg_from_pat_type(pat_type: &syn::PatType) -> Option<sig::Arg> {
        let ident =
            if let syn::Pat::Ident(ref pat_ident) = pat_type.pat.as_ref() {
                pat_ident.ident.clone()
            } else {
                panic!("Can't handle non-ident arg pattern");
            };
        let ty = (*pat_type.ty).clone();
        Some(sig::Arg { ident, ty })
    }
}

mod gen {
    //! Generate code from contract signature

    use super::err;
    use super::sig;
    use groundhog_xdr::contract::ContractEntrypointID;
    use groundhog_xdr::serialize::{Limits, WriteXdr};
    use groundhog_xdr::types::MethodID;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::format_ident;
    use quote::{quote, quote_spanned};
    use syn::parse::Result;
    use syn::spanned::Spanned;

    // Generate a module `client` that contains the client type
    // (a wrapper around a contract address) and an implementation
    // of the contract trait that performs cross-contract calls.
    pub fn client_from_contract_signature(
        contract: &sig::Contract,
        config: &sig::Config,
    ) -> Result<TokenStream2> {
        let trait_ident = &contract.ident;
        let client_ident = format_ident!("{}Client", trait_ident);
        let client_mod_ident = config.client_mod_ident();

        let mut method_stubs = TokenStream2::new();
        let mut methods: Vec<_> = contract.methods.iter().collect();
        methods.sort_by(|a, b| a.ident.to_string().cmp(&b.ident.to_string()));

        for (index, method) in methods.iter().enumerate() {
            method_stubs.extend(call_from_method_signature(
                get_method_num(
                    &trait_ident.to_string(),
                    "0.1.0",
                    index.try_into().unwrap(),
                ),
                method,
            )?);
        }

        Ok(quote! {

            pub mod #client_mod_ident {
                use ::sdk::types::ContractAddress;
                use ::sdk::types::RuntimeMemoryLength;
                use ::sdk::internal::invoke;

                use super::#trait_ident;

                // The below implementation may require other public definitions
                // that the trait definition used
                use super::*;

                pub struct #client_ident {
                    address: ContractAddress
                }

                impl #client_ident {
                    pub fn new(address: &ContractAddress) -> Self {
                        Self { address: address.clone() }
                    }
                }

                impl #trait_ident for #client_ident {
                    #method_stubs
                }
            }

        })
    }

    fn call_from_method_signature(
        method_index: MethodID,
        method: &syn::Signature,
    ) -> Result<TokenStream2> {
        let ident = &method.ident;
        let mut params = method.inputs.iter();

        // Pop the "receiver" parameter
        let receiver_param = {
            if let Some(p) = params.next() {
                Ok(p)
            } else {
                err(method, "No receiver param in contract method")
            }
        }
        .unwrap();
        let receiver_arg = get_arg_name(receiver_param)?;
        let address = if is_receiver(receiver_param) {
            quote! { &#receiver_arg.address }
        } else {
            // It's an Option<&mut Self> pseudo-receiver
            quote! { &#receiver_arg.unwrap().address }
        };
        let sent_params: Vec<&syn::FnArg> = params.collect();
        let sent_args: Vec<&syn::Pat> =
            sent_params.iter().cloned().filter_map(arg_pat).collect();
        let args_tuple = if sent_args.len() == 1 {
            let arg = sent_args[0];
            quote! { #arg }
        } else {
            quote! { ( #(#sent_args),* ) }
        };
        let return_type = &method.output;
        let gas_limit = u64::MAX;

        let export_name = get_method_name(method_index);

        let comment_text = format!("/// Export as {}", export_name);
        let comment_tokens: proc_macro2::TokenStream =
            comment_text.parse().unwrap();

        let print_string = format!("[sdk] Client: Calling method {}", ident);

        let log_call = quote! { 
            ::sdk::debug_log!("{} with args {:?}", #print_string, #args_tuple); 
        };

        Ok(quote_spanned! {method.span()=>

            fn #ident(#receiver_param, #(#sent_params),* ) #return_type
            {
                #comment_tokens
                let args = #args_tuple;
                #log_call
                invoke(#address, #method_index, #gas_limit, &args)
            }

        })
    }

    fn get_arg_name(fn_arg: &syn::FnArg) -> Result<TokenStream2> {
        match fn_arg {
            syn::FnArg::Receiver(ref r) => {
                let token = &r.self_token;
                Ok(quote! { #token })
            }
            syn::FnArg::Typed(ref pat_type) => {
                if let syn::Pat::Ident(ref pat_ident) = pat_type.pat.as_ref() {
                    let ident = &pat_ident.ident;
                    Ok(quote! { #ident })
                } else {
                    super::err(fn_arg, "Can't handle non-ident arg pattern")
                }
            }
        }
    }

    fn is_receiver(fn_arg: &syn::FnArg) -> bool {
        matches!(fn_arg, syn::FnArg::Receiver(_))
    }

    fn arg_pat(fn_arg: &syn::FnArg) -> Option<&syn::Pat> {
        match fn_arg {
            syn::FnArg::Typed(pat_type) => Some(&pat_type.pat),
            syn::FnArg::Receiver(_) => None,
        }
    }

    pub fn test_registration_from_method_signature(
        contract: &sig::ContractImpl,
        config: &sig::Config,
    ) -> Result<TokenStream2> {
        let test_setup_mod_ident = config.test_setup_mod_ident();
        let exports_mod_ident = config.exports_mod_ident();
        let mut registrations = TokenStream2::new();
        let mut index: u16 = 0;
        for method in &contract.methods {
            let method_id = get_method_num(
                &contract.trait_ident.to_string(),
                "0.1.0",
                index,
            );
            registrations.extend(registration_from_method_signature(
                method, method_id,
            )?);
            index += 1;
        }
        let mock_registrations = registrations.clone();

        Ok(quote! {

            #[cfg(feature="mock")]
            pub mod #test_setup_mod_ident {
                use super::#exports_mod_ident::*;

                #[allow(dead_code)]
                pub fn register(env : &mut ::sdk::TestEnvironment) -> ::sdk::types::Contract {
                    let mut contract = env.add_contract();

                    #registrations

                    contract.as_contract()
                }

                pub fn register_mock(contract: &mut ::sdk::MockContract<'_>) {
                    #mock_registrations
                }
            }

        })
    }

    pub fn exports_from_contract_signature(
        contract: &sig::ContractImpl,
        config: &sig::Config,
    ) -> Result<TokenStream2> {
        let self_ty = &contract.self_ty;
        // let generics = &contract.generics;

        let exports_mod_ident = config.exports_mod_ident();
        let mut exports = TokenStream2::new();
        let mut methods: Vec<_> = contract.methods.iter().collect();
        methods.sort_by(|a, b| a.ident.to_string().cmp(&b.ident.to_string()));
        for (index, method) in methods.iter().enumerate() {
            let method_id = get_method_num(
                &contract.trait_ident.to_string(),
                "0.1.0",
                index.try_into().unwrap(),
            );
            exports.extend(export_from_method_signature(
                Some(contract),
                method,
                method_id,
            )?);
        }

        let use_trait = if let Some(trait_) = &contract.trait_ {
            quote! { use super::#trait_; }
        } else {
            quote! {}
        };

        Ok(quote! {

            pub mod #exports_mod_ident {
                #use_trait
                use super::#self_ty;
                use ::sdk::storage::{Keypath, LoadKeypath};
                use ::sdk::serialize::Serialize;

                #exports
            }

        })
    }

    pub fn export_from_method_signature(
        contract: Option<&sig::ContractImpl>,
        method: &sig::Method,
        export_num: MethodID,
    ) -> Result<TokenStream2> {
        let ident = &method.ident;
        let export_name = get_method_name(export_num);
        let extern_name = format_ident!("{}_extern", ident);
        let internal_name = format_ident!("{}_internal", ident);
        let export_num_name =
            format_ident!("{}_EXPORT_NUM", ident.to_string().to_uppercase());

        let mut export = quote! {

            #[cfg_attr(target_family = "wasm", unsafe(export_name = #export_name))]
            pub extern "C" fn #extern_name() -> i32 {
                #internal_name()
            }
        };

        if let Some(contract) = contract {
            let self_ty = &contract.self_ty;

            if method.receiver.is_some() {
                let provided_args = provided_args(method, false);
                let received_args = received_args(method, false);

                // Load the contract object and call the method on it
                export.extend(quote! {

                    pub fn #internal_name() -> i32 {
                        ::sdk::internal::handle_call(| #received_args | {
                            let mut state = <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                                .expect("Failed loading state")
                                .expect("Contract state not present");
                            state.#ident( #provided_args )
                        }).into()
                    }

                });
            } else {
                let provided_args = provided_args(method, true);
                let received_args = received_args(method, true);
                let self_ty_full = if let Some(trait_) = &contract.trait_ {
                    quote! { <#self_ty as #trait_> }
                } else {
                    quote! { #self_ty }
                };

                // Call the associated function (without pre-loading contract state)
                export.extend(quote! {

                    pub fn #internal_name() -> i32 {
                        ::sdk::internal::handle_call(| #received_args | {
                            let mut maybe_state = <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                                .expect("Failed loading state");
                            let state_ref = if let Some(ref mut state) = &mut maybe_state {
                                Some(state)
                            } else {
                                None
                            };
                            #self_ty_full::#ident(state_ref, #provided_args )
                        }).into()
                    }

                });
            }
        } else {
            let provided_args = provided_args(method, false);
            let received_args = received_args(method, false);

            // Call the free function
            export.extend(quote! {

                pub fn #internal_name() -> i32 {
                    ::sdk::internal::handle_call(| #received_args | {
                        #ident( #provided_args )
                    }).into()
                }

                pub const #export_num_name : u32 = #export_num;

            });
        }

        Ok(export)
    }

    fn registration_from_method_signature(
        method: &sig::Method,
        export_num: MethodID,
    ) -> Result<TokenStream2> {
        let ident = &method.ident;
        let internal_name = format_ident!("{}_internal", ident);

        Ok(quote! {

            contract.register_method(#export_num, #internal_name);

        })
    }

    pub fn get_method_num(
        trait_name: &str,
        version_str: &str,
        method_index: u16,
    ) -> MethodID {
        use sha2::Digest;

        let preimage =
            ::groundhog_xdr::contract::ContractInterfaceIDPreimage {
                name: trait_name.to_string().as_bytes().try_into().unwrap(),
                vers: version_str.to_string().as_bytes().try_into().unwrap(),
            };

        let mut hasher = sha2::Sha256::new();
        let preimage_bytes = preimage.to_xdr(Limits::none()).unwrap();
        hasher.update(preimage_bytes);
        let result = hasher.finalize();

        let prefix: &[u8; 6] = &result[0..6].try_into().unwrap();

        let id = ContractEntrypointID {
            contract_interface_id: prefix.clone().try_into().unwrap(),
            contract_method_id: method_index.to_le_bytes().into(),
        };

        let mut id_bytes: [u8; 8] = [0; 8];
        id_bytes[0..6].copy_from_slice(id.contract_interface_id.as_slice());
        id_bytes[6..8].copy_from_slice(id.contract_method_id.as_slice());

        MethodID::from_le_bytes(id_bytes)
    }

    fn get_method_name(num: MethodID) -> String {
        format!("{:X}", num)
    }

    fn provided_args(method: &sig::Method, drop_first: bool) -> TokenStream2 {
        let skip_n = if drop_first { 1 } else { 0 };
        let args = method.args.iter().skip(skip_n);

        let provided_args: Vec<TokenStream2> =
            args.map(provided_arg).collect();
        quote! { #(#provided_args),* }
    }

    fn provided_arg(arg: &sig::Arg) -> TokenStream2 {
        let ident = &arg.ident;
        let ty = &arg.ty;
        match ty {
            // If the interface expects a reference ...
            syn::Type::Reference(_) =>
            // ... pass a reference into the deserialized calldata
            {
                quote! { &#ident }
            }
            _ =>
            // Else, pass the owned value
            {
                quote! { #ident }
            }
        }
    }

    fn received_args(method: &sig::Method, drop_first: bool) -> TokenStream2 {
        let skip_n = if drop_first { 1 } else { 0 };
        let args = method.args.iter().skip(skip_n);

        let received_args: Vec<TokenStream2> =
            args.map(received_arg).collect();
        quote! { ( #(#received_args),* ) }
    }

    fn received_arg(arg: &sig::Arg) -> TokenStream2 {
        let ident = &arg.ident;
        /*
        let ty = &arg.ty;
        let ty_no_ref = match ty {
            syn::Type::Reference(ref_ty) => {
                // Strip the reference from the argument type
                &*ref_ty.elem
            }
            _ => ty,
        };
        */
        quote! { #ident }
    }
}
