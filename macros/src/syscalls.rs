use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::collections::HashMap;

#[derive(Debug, serde::Deserialize)]
struct SyscallSpec {
    type_mappings: Vec<TypeMapping>,
    syscalls: Vec<Syscall>,
    errors: Vec<SyscallErrorDecl>,
}

#[derive(Debug, serde::Deserialize)]
struct SyscallErrorDecl {
    name: String,
    number: i32,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, serde::Deserialize)]
struct Syscall {
    name: String,
    number: i32,
    inputs: Vec<Arg>,
    outputs: Vec<Arg>,
    #[serde(default = "default_true")]
    returns: bool, // default to true
}

#[derive(Debug, serde::Deserialize)]
struct TypeMapping {
    base_name: String,
    rust_name: String,
    xdr_name: String,
}

struct TypeInfo {
    rust_name: String,
    xdr_name: String,
}

type TypeMap = HashMap<String, TypeInfo>;

impl SyscallSpec {
    pub fn get_type_map(&self) -> TypeMap {
        let mut out = HashMap::new();
        for elt in &self.type_mappings {
            out.insert(
                elt.base_name.clone(),
                TypeInfo {
                    rust_name: elt.rust_name.clone(),
                    xdr_name: elt.xdr_name.clone(),
                },
            );
        }
        out
    }
}

impl Syscall {
    pub fn return_type(&self) -> TokenStream2 {
        if self.returns {
            return quote! {
                SyscallResult<()>
            };
        }
        quote! {
            !
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct Arg {
    name: String,
    span_argument: Option<bool>,
    typename: Option<String>,
    rust_typename: Option<String>,
    #[serde(default)]
    primitive_type: bool, // default to false
    #[serde(default)]
    optional: bool, // default to false
}

impl Arg {
    fn is_span_argument(&self) -> bool {
        if let Some(x) = self.span_argument {
            return x;
        }
        return false;
    }
    fn formatted_name(&self) -> proc_macro2::Ident {
        syn::Ident::new(&self.name, proc_macro2::Span::call_site())
    }

    fn get_type_details<'a>(
        &self,
        type_map: &'a TypeMap,
    ) -> Option<&'a TypeInfo> {
        match &self.typename {
            None => {
                return None;
            }
            Some(name) => return type_map.get(&name.clone()),
        }
    }

    fn fixed_size_typename(&self, type_map: &TypeMap) -> Option<String> {
        match &self.typename {
            None => {
                return None;
            }
            _ => {}
        };

        if let Some(type_details) = self.get_type_details(type_map) {
            return Some(type_details.rust_name.clone());
        }
        if let Some(x) = &self.rust_typename {
            return Some(x.clone());
        }
        self.typename.clone()
    }

    pub fn to_u64s(&self, type_map: &TypeMap) -> Vec<TokenStream2> {
        let name = self.formatted_name();

        if self.is_span_argument() {
            return vec![
                quote! {
                    #name.to_u64_0(),
                },
                quote! {
                    #name.to_u64_1(),
                },
            ];
        }

        if let Some(_) = self.fixed_size_typename(type_map) {
            return vec![quote! {
                #name.to_u64_0(),
            }];
        }

        unimplemented!("Unknown arg type");
    }

    // does not include trailing comma
    pub fn to_uninit_input(
        &self,
        is_input: bool,
        type_map: &TypeMap,
    ) -> TokenStream2 {
        let name = self.formatted_name();
        if self.is_span_argument() {
            if is_input {
                return quote! {
                    #name : &[u8]
                };
            }
            return quote! {
                #name :  &mut [MaybeUninit<u8>]
            };
        }
        if let Some(type_str) = self.fixed_size_typename(type_map) {
            let ty = format_ident!("{}", type_str);
            if is_input {
                if self.primitive_type {
                    return quote! {
                        #name : #ty
                    };
                }
                if self.optional {
                    return quote! {
                        #name : Option<&#ty>
                    };
                }
                return quote! {
                    #name : &#ty
                };
            }
            if self.primitive_type {
                return quote! {
                    #name : &mut #ty
                };
            }
            return quote! {
                #name : &mut MaybeUninit<#ty>
            };
        }

        unimplemented!("Unknown arg type");
    }

    // does not include trailing comma
    pub fn to_raw_input(
        &self,
        is_input: bool,
        type_map: &TypeMap,
    ) -> TokenStream2 {
        let name = self.formatted_name();

        let ty = if self.is_span_argument() {
            quote! { [u8] }
        } else if let Some(type_str) = self.fixed_size_typename(type_map) {
            let ident = format_ident!("{}", type_str);
            quote! { #ident }
        } else {
            unimplemented!("Unknown arg type");
        };

        if is_input {
            if self.primitive_type {
                return quote! {
                    #name : #ty
                };
            }
            if self.optional {
                return quote! {
                    #name : Option<&#ty>
                };
            }
            return quote! {
                #name : &#ty
            };
        }
        return quote! {
            #name : &mut #ty
        };
    }

    // raw_SYSCALL wraps uninit_SYSCALL, getting rid of the MaybeUninit usage.
    // These return the arguments forwarded to the uninit_SYSCALL call.
    pub fn to_raw_through_call(
        &self,
        is_input: bool,
        type_map: &TypeMap,
    ) -> TokenStream2 {
        let name = self.formatted_name();

        if is_input || self.primitive_type {
            return quote! { #name };
        }
        if self.is_span_argument() {
            return quote! {

                unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(#name)}
            };
        }
        if let Some(type_str) = self.fixed_size_typename(type_map) {
            let ty = format_ident!("{}", type_str);
            return quote! {
                unsafe {core::mem::transmute::<&mut #ty, &mut MaybeUninit<#ty>>(#name)}
            };
        }

        unimplemented!("Unknown arg type");
    }
}

fn generate_uninit_syscall_inputs(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_uninit_input(true, type_map))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_uninit_input(false, type_map))
        .collect();

    args.extend(output_args);

    args
}

fn generate_raw_through_calls(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_raw_through_call(true, type_map))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_raw_through_call(false, type_map))
        .collect();

    args.extend(output_args);

    args
}

fn generate_raw_syscall_inputs(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_raw_input(true, type_map))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_raw_input(false, type_map))
        .collect();

    args.extend(output_args);

    args
}

fn generate_u64_syscall_args(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> Vec<TokenStream2> {
    // inputs
    let mut u64_args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .flat_map(|inp| inp.to_u64s(type_map))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .flat_map(|outp| outp.to_u64s(type_map))
        .collect();

    u64_args.extend(output_args);

    const MAX_SYSCALL_ARGS: usize = 6;

    if u64_args.len() > MAX_SYSCALL_ARGS {
        return vec![quote! {
            compile_error!("Too many syscall args")
        }];
    }

    u64_args.resize(MAX_SYSCALL_ARGS, quote! { 0, });

    u64_args
}

fn generate_syscall_uninit_decl(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> TokenStream2 {
    //  pub fn uninit_SHA256(buffer : &[u8], hash : &mut MaybeUninit<SHA256Hash>) -> SyscallResult<()> {
    //
    //      // syscall will initialize outputs, if error is kNone
    //      let err: SysErr =
    //          syscall_wrapper(SyscallNo::kSHA256,
    //              buffer.as_ptr() as u64,
    //              buffer.len() as u64,
    //              hash.as_mut_ptr() as u64,
    //              0,
    //              0,
    //              0);
    //
    //      if !err.is_none() {
    //          return Err(err);
    //      }
    //
    //      Ok(())
    //  }

    let return_type = syscall.return_type();
    let post_syscall = if syscall.returns {
        quote! {
            if !err.is_none() {
                return Err(err);
            }

            Ok(())
        }
    } else {
        quote! {
            unsafe {unreachable_unchecked()}
        }
    };

    let u64_args = generate_u64_syscall_args(&syscall, type_map);
    let uninit_syscall_inputs =
        generate_uninit_syscall_inputs(&syscall, type_map);

    let uninit_ident = format_ident!("uninit_{}", &syscall.name);
    let syscall_no = format_ident!("k{}", &syscall.name);

    quote! {
        pub fn #uninit_ident(#(#uninit_syscall_inputs,)*) -> #return_type {

            // syscall will initialize outputs, if error is kNone
            let err: SysErr =
                syscall_wrapper(SyscallNo::#syscall_no,
                    #(#u64_args)*);

            #post_syscall
        }
    }
}

fn generate_syscall_raw_decl(
    syscall: &Syscall,
    type_map: &TypeMap,
) -> TokenStream2 {
    let raw_syscall_inputs = generate_raw_syscall_inputs(&syscall, type_map);
    let forward_args = generate_raw_through_calls(&syscall, type_map);

    let uninit_ident = format_ident!("uninit_{}", &syscall.name);
    let raw_ident = format_ident!("raw_{}", &syscall.name);

    let return_type = syscall.return_type();

    quote! {
        pub fn #raw_ident(#(#raw_syscall_inputs,)*) -> #return_type {
            #uninit_ident(#(#forward_args,)*)
        }
    }
}

fn load_syscall_yaml(file_path: String) -> SyscallSpec {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let yaml_path = std::path::PathBuf::from(manifest_dir).join(&file_path);

    let content = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", yaml_path.display()));

    serde_yaml::from_str(&content).unwrap()
}

pub fn generate_syscall_decls(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();
    let spec = load_syscall_yaml(file_path);
    let type_map = spec.get_type_map();

    let decls = spec.syscalls.iter().map(|func| {
        let uninit = generate_syscall_uninit_decl(&func, &type_map);
        let raw = generate_syscall_raw_decl(&func, &type_map);
        quote! {
            #uninit
            #raw
        }
    });

    quote! {
        #(#decls)*
    }
    .into()
}

fn generate_syscall_no_line(syscall: &Syscall) -> TokenStream2 {
    let name = format_ident!("k{}", &syscall.name);
    let num = syscall.number;
    quote! {
        #name = #num as i32
    }
}

pub fn generate_syscall_numbers(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();
    let spec = load_syscall_yaml(file_path);

    let lines = spec
        .syscalls
        .iter()
        .map(|func| generate_syscall_no_line(&func));

    quote! {
        #[repr(i32)]
        #[derive(Eq,PartialEq,Copy,Debug,Clone)]
        pub enum SyscallNo {
            #(#lines,)*
        }
    }
    .into()
}

fn generate_syscall_error_code_line(error: &SyscallErrorDecl) -> TokenStream2 {
    let name = format_ident!("k{}", &error.name);
    let num = error.number;
    quote! {
        #name = #num as i32
    }
}

pub fn generate_syscall_error_codes(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();
    let spec = load_syscall_yaml(file_path);

    let lines = spec
        .errors
        .iter()
        .map(|err| generate_syscall_error_code_line(&err));

    quote! {
        #[repr(i32)]
        #[derive(Eq,PartialEq,Copy,Debug,Clone)]
        pub enum SyscallErrorCode {
            #(#lines,)*
        }
    }
    .into()
}

fn generate_error_code_helper_line(error: &SyscallErrorDecl) -> TokenStream2 {
    let name = format_ident!("k{}", &error.name);
    quote! {
        e if e == SyscallErrorCode::#name as i32 => Ok(SyscallErrorCode::#name)
    }
}

pub fn generate_syscall_error_helper(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();
    let spec = load_syscall_yaml(file_path);

    let lines = spec
        .errors
        .iter()
        .map(|err| generate_error_code_helper_line(&err));

    quote! {
        impl TryInto<SyscallErrorCode> for SysErr {
            type Error = i32;

            fn try_into(self) -> Result<SyscallErrorCode, Self::Error> {
                match self.0 {
                     #(#lines,)*

                     e => Err(e)
                }
            }
        }
    }
    .into()
}
