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
            Err(e) => return e.write_errors().into(),
        },
        Err(e) => {
            return darling::Error::from(e).write_errors().into();
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
                gen::test_registration_from_contract_signature(&sig, config)?;
            out.extend(test_registration.clone());

            Ok(out)
        }
        syn::Item::Fn(item) => {
            let sig = parse::method_signature_from_sig(&item.sig);
            let mut out = TokenStream2::new();

            let entrypoint = gen::get_method_num(None, None, &item.sig.ident, 0);
            out.extend(gen::export_from_method_signature(
                None, &sig, entrypoint, config,
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
        pub storage: Option<String>,
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
    #[allow(dead_code)]
    pub struct Arg {
        pub ident: syn::Ident,
        pub ty: syn::Type,
    }
}

mod sig_impls {
    use super::sig::*;
    use proc_macro2::Span;

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

        pub fn storage(&self) -> bool {
            if let Some(setting) = &self.storage {
                setting.as_str() == "true"
            } else {
                true // default if no storage attr present
            }
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

    impl Contract {
        pub fn interface_trait_ident(&self) -> Option<&syn::Ident> {
            Some(&self.ident)
        }
    }

    impl ContractImpl {
        pub fn interface_trait_ident(&self) -> Option<&syn::Ident> {
            self.trait_.as_ref().map(path_last_segment_ident)
        }
    }

    fn path_last_segment_ident(path: &syn::Path) -> &syn::Ident {
        let seg = path.segments.last().unwrap();
        &seg.ident
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
    use xdrs::serialize_txs;
    use groundhog_xdr::contract::ContractEntrypointID;
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
            let entrypoint = get_method_num(
                contract.interface_trait_ident(),
                None,
                &method.ident,
                index
            );
            method_stubs.extend(call_from_method_signature(
                entrypoint,
                method,
                config,
            )?);
        }

        Ok(quote! {

            pub mod #client_mod_ident {
                use ::sdk::types::ContractAddress;
                use ::sdk::types::ContractEntrypointID;
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
        entrypoint: ContractEntrypointID,
        method: &syn::Signature,
        _config: &sig::Config,
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

        let export_name = get_method_name(entrypoint);
        let entrypoint_lit = entrypoint_literal(entrypoint);

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
                let entrypoint = #entrypoint_lit;
                let args = #args_tuple;

                #log_call

                invoke(#address, entrypoint, #gas_limit, &args)
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

    pub fn test_registration_from_contract_signature(
        contract: &sig::ContractImpl,
        config: &sig::Config,
    ) -> Result<TokenStream2> {
        let test_setup_mod_ident = config.test_setup_mod_ident();
        let exports_mod_ident = config.exports_mod_ident();
        let mut registrations = TokenStream2::new();

        let mut methods: Vec<_> = contract.methods.iter().collect();
        methods.sort_by(|a, b| a.ident.to_string().cmp(&b.ident.to_string()));
        for (index, method) in methods.iter().enumerate() {
            let method_id = get_method_num(
                contract.interface_trait_ident(),
                None,
                &method.ident,
                index
                );
            registrations.extend(registration_from_method_signature(
                method, method_id,
            )?);
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
            let entrypoint = get_method_num(
                contract.interface_trait_ident(),
                None,
                &method.ident,
                index);
            exports.extend(export_from_method_signature(
                Some(contract),
                method,
                entrypoint,
                config,
            )?);
        }

        let use_trait = if let Some(trait_) = &contract.trait_ {
            quote! { use super::#trait_; }
        } else {
            quote! {}
        };

        let use_storage = if config.storage() {
            quote! {
                use ::sdk::storage::{Keypath, LoadKeypath, DefaultFromKeypath, Store};
            }
        } else {
            quote! {}
        };

        Ok(quote! {

            pub mod #exports_mod_ident {
                #use_trait
                use super::#self_ty;
                #use_storage
                use ::sdk::serialize::Serialize;
                use ::sdk::types::ContractEntrypointID;
                use ::sdk::contract_panic;

                #exports
            }

        })
    }

    // Generate an exported function for:
    // - An impl or impl-trait method with self-parameter
    // - An impl or impl-trait associated function
    // - A free function
    pub fn export_from_method_signature(
        contract: Option<&sig::ContractImpl>,
        method: &sig::Method,
        entrypoint: ContractEntrypointID,
        config: &sig::Config,
    ) -> Result<TokenStream2> {
        let ident = &method.ident;
        let export_name = get_method_name(entrypoint);
        let extern_name = format_ident!("{}_extern", ident);
        let internal_name = format_ident!("{}_internal", ident);
        let export_num_name =
            format_ident!("{}_EXPORT_NUM", ident.to_string().to_uppercase());

        let print_string = format!("[sdk] Guest: Calling method {}", ident);
        let log_guest_call = quote! { 
            ::sdk::debug_log!("{} ", #print_string); 
        };

        let mut export = quote! {
            #[cfg_attr(target_family = "wasm", unsafe(export_name = #export_name))]
            pub extern "C" fn #extern_name() -> () {
                #internal_name()
            }
        };

        let arg_names = arg_names(method);

        // The function is a method that takes a self-parameter
        let is_method = contract.is_some() && method.receiver.is_some();

        // The function is associated with a type
        let is_associated = contract.is_some();

        // How to call the function
        let function_or_method = if is_method {
            // Invoke the method on a state object
            quote! { state.#ident }
        } else if is_associated {
            // Invoke the associated function on the contract type
            let self_ty = &contract.unwrap().self_ty;
            quote! { #self_ty::#ident }
        } else {
            // Invoke the free function
            quote! { #ident }
        };

        // The method takes a mutable self-parameter
        let is_mut = is_method && method.receiver.as_ref().map(|r| r.mutability.is_some()).unwrap_or(false);

        // The method is marked "init": storage should be uninitialized before calling
        // TODO: use a method attribute to mark initialization methods
        let is_init = is_method && ident.to_string().as_str() == "initialize";

        let assert_uninitialized = if is_method && config.storage() && is_init {
            let self_ty = &contract.unwrap().self_ty;

            quote! {
                // Assert that the storage does not yet exist
                if <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                    .unwrap_or_else(|_| contract_panic!("Failed loading contract storage")).is_some()
                {
                    contract_panic!("#[contract(init)] method called when contract storage already exists");
                }
            }
        } else {
            quote! {}
        };

        // Load the stored contract state (when appropriate)
        let load_state = if is_method {
            let self_ty = &contract.unwrap().self_ty;
            let mutability = if is_mut { quote! { mut } } else { quote! {} };

            if config.storage() {
                if is_init {
                    // Get a default value to initialize in this method
                    quote! {
                        let #mutability state =
                            <#self_ty as DefaultFromKeypath>::default_from_keypath(&Keypath::root());
                    }
                } else {
                    // Load the previously-stored value
                    quote! {
                        let #mutability state =
                            <#self_ty as LoadKeypath>::try_load_keypath(&Keypath::root())
                                .unwrap_or_else(|_| contract_panic!("Failed loading contract storage"))
                                .unwrap_or_else(|| contract_panic!("Contract storage not present"));
                    }
                }
            } else {
                // If not using storage, we just create a default object to call the method on
                quote! {
                    let #mutability state = <#self_ty as Default>::default();
                }
            }
        } else {
            // Free function: no state variable at all
            quote! {}
        };

        // Save the contract state after calling the method, if it takes a mutable ref to the state
        let save_state = if is_method && config.storage() && is_mut {
            quote! {
                state.store_keypath(&Keypath::root()).unwrap_or_else(|_|
                    contract_panic!("Failed saving contract storage"));
            }
        } else {
            quote! {}
        };

        let use_alloc = if cfg!(feature = "alloc") {
            quote! {
                extern crate alloc;
                use alloc::boxed::Box;
            }
        } else { quote! {} };

        export.extend(quote! {
            pub fn #internal_name() -> () {
                use sdk::invoke::{contract_return, get_calldata_sized};
                use sdk::serialize::{encode_xdr, decode_xdr};
                #use_alloc

                // Debug mode: log the method-call
                #log_guest_call

                #assert_uninitialized

                #load_state

                const CALLDATA_BUFSZ = 1000;
                let calldata: [u8, CALLDATA_BUFSZ] = get_calldata_sized().unwrap_or_else(|e|
                    sdk::contract_panic!("get_calldata() failed"));

                // Decode the arguments expected by the method
                let (#arg_names) = decode_xdr(call_data.as_ref()).unwrap_or_else(|_|
                    sdk::contract_panic!("failed decoding contract-call arguments"));

                // Call the free function or contract-type method
                let result = #function_or_method(#arg_names);

                // Save any mutations
                #save_state

                // Encode the result
                let result_bytes = encode_xdr(&result).unwrap_or_else(|_|
                    sdk::contract_panic!("failed encoding contract-call result"));

                // Return the contract-call result to the VM
                contract_return(result_bytes.as_slice());

                // notreached
            }
        });

        // A hack for test setup
        if contract.is_none() && cfg!(feature = "alloc") {
            let entrypoint_lit = entrypoint_literal(entrypoint);

            export.extend(quote! {
                pub const #export_num_name : ContractEntrypointID = #entrypoint_lit;
            });
        }

        Ok(export)
    }

    fn registration_from_method_signature(
        method: &sig::Method,
        entrypoint: ContractEntrypointID,
    ) -> Result<TokenStream2> {
        let ident = &method.ident;
        let internal_name = format_ident!("{}_internal", ident);
        let entrypoint = entrypoint_literal(entrypoint);

        Ok(quote! {

            contract.register_method(#entrypoint, #internal_name);

        })
    }

    pub fn get_method_num(
        trait_ident: Option<&syn::Ident>,
        version_str: Option<&str>,
        method_ident: &syn::Ident,
        method_index: usize,
    ) -> ContractEntrypointID {
        use sha2::Digest;

        let iface_ident = trait_ident.unwrap_or(method_ident);
        let iface_name = iface_ident.to_string();
        let version_str = version_str.unwrap_or("0.0.0");
        let method_index: u16 = method_index.try_into().unwrap();

        // println!("METHOD if:<{}> v:<{}> ix:<{}>", iface_name, version_str, method_index);

        let preimage =
            ::groundhog_xdr::contract::ContractInterfaceIDPreimage {
                name: iface_name.as_bytes().try_into().unwrap(),
                vers: version_str.as_bytes().try_into().unwrap(),
            };

        let mut hasher = sha2::Sha256::new();
        let preimage_bytes = serialize_txs::encode(&preimage).unwrap();
        hasher.update(preimage_bytes);
        let result = hasher.finalize();

        let interface_id: &[u8; 6] = &result[0..6].try_into().unwrap();
        let method_id: [u8; 2] = method_index.to_be_bytes().into();

        let mut entrypoint_id: [u8; 8] = [0; 8];
        entrypoint_id[0..6].copy_from_slice(interface_id);
        entrypoint_id[6..8].copy_from_slice(&method_id);

        ContractEntrypointID::from(entrypoint_id)
    }

    fn get_method_name(id: ContractEntrypointID) -> String {
        let mut s = String::new();
        let bytes: &[u8] = id.as_ref();
        let method : u64 = u64::from_be_bytes(bytes.try_into().unwrap());
        s.push_str(&format!("{:X}", method));
        s
    }

    fn entrypoint_literal(entrypoint: ContractEntrypointID) -> TokenStream2 {
        let bytes: &[u8] = entrypoint.as_ref();
        quote!(
            sdk::types::ContractEntrypointID::new([#(#bytes),*])
        )
    }

    fn arg_names(method: &sig::Method) -> TokenStream2 {
        let args: Vec<TokenStream2> = method.args.iter().map(|arg| {
            let ident = &arg.ident;
            quote! { #ident }
        }).collect();

        quote! { #( #args ),* }
    }
}
