use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

#[derive(Debug, serde::Deserialize)]
struct SyscallSpec {
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

    fn fixed_size_typename(&self) -> Option<String> {
        if let Some(x) = &self.rust_typename {
            return Some(x.clone());
        }
        self.typename.clone()
    }

    pub fn to_u64s(&self, is_input: bool) -> Vec<TokenStream2> {
        let name = self.formatted_name();

        if self.is_span_argument() {
            if is_input {
                return vec![
                    quote! {
                        #name.as_ptr() as u64,
                    },
                    quote! {
                        #name.len() as u64,
                    },
                ];
            }
            return vec![
                quote! {
                    #name.as_mut_ptr() as u64,
                },
                quote! {
                    #name.len() as u64,
                },
            ];
        }

        if let Some(_) = self.fixed_size_typename() {
            if self.primitive_type {
                if is_input {
                    // passed directly in the u64
                    return vec![quote! {
                        #name as u64,
                    }];
                }
                // primitive types in output are still passed as references
                return vec![quote! {
                    to_offset_mut(#name) as u64,
                }];
            }

            if is_input {
                return vec![quote! {
                    #name.as_ptr() as u64,
                }];
            }
            return vec![quote! {
                #name.as_mut_ptr() as u64,
            }];
        }

        unimplemented!("Unknown arg type");
    }

    // does not include trailing comma
    pub fn to_uninit_input(&self, is_input: bool) -> TokenStream2 {
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
        if let Some(type_str) = self.fixed_size_typename() {
            let ty = format_ident!("{}", type_str);
            if is_input {
                if self.primitive_type {
                    return quote! {
                        #name : #ty
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
    pub fn to_raw_input(&self, is_input: bool) -> TokenStream2 {
        let name = self.formatted_name();

        let ty = if self.is_span_argument() {
            quote! { [u8] }
        } else if let Some(type_str) = self.fixed_size_typename() {
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
    pub fn to_raw_through_call(&self, is_input: bool) -> TokenStream2 {
        let name = self.formatted_name();

        if is_input || self.primitive_type {
            return quote! { #name };
        }
        if self.is_span_argument() {
            return quote! {

                unsafe {core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(#name)}
            };
        }
        if let Some(type_str) = &self.typename {
            let ty = format_ident!("{}", type_str);
            return quote! {
                unsafe {core::mem::transmute::<&mut #ty, &mut MaybeUninit<#ty>>(#name)}
            };
        }

        unimplemented!("Unknown arg type");
    }
}

fn generate_uninit_syscall_inputs(syscall: &Syscall) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_uninit_input(true))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_uninit_input(false))
        .collect();

    args.extend(output_args);

    args
}

fn generate_raw_through_calls(syscall: &Syscall) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_raw_through_call(true))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_raw_through_call(false))
        .collect();

    args.extend(output_args);

    args
}

fn generate_raw_syscall_inputs(syscall: &Syscall) -> Vec<TokenStream2> {
    // inputs
    let mut args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .map(|inp| inp.to_raw_input(true))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .map(|outp| outp.to_raw_input(false))
        .collect();

    args.extend(output_args);

    args
}

fn generate_u64_syscall_args(syscall: &Syscall) -> Vec<TokenStream2> {
    // inputs
    let mut u64_args: Vec<TokenStream2> = syscall
        .inputs
        .iter()
        .flat_map(|inp| inp.to_u64s(true))
        .collect();

    // outputs
    let output_args: Vec<TokenStream2> = syscall
        .outputs
        .iter()
        .flat_map(|outp| outp.to_u64s(false))
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

fn generate_syscall_uninit_decl(syscall: &Syscall) -> TokenStream2 {
    //  #[cfg(not(feature="mock"))]
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

    let u64_args = generate_u64_syscall_args(&syscall);
    let uninit_syscall_inputs = generate_uninit_syscall_inputs(&syscall);

    let uninit_ident = format_ident!("uninit_{}", &syscall.name);
    let syscall_no = format_ident!("k{}", &syscall.name);

    quote! {
        #[cfg(not(feature="mock"))]
        pub fn #uninit_ident(#(#uninit_syscall_inputs,)*) -> #return_type {

            // syscall will initialize outputs, if error is kNone
            let err: SysErr =
                syscall_wrapper(SyscallNo::#syscall_no,
                    #(#u64_args)*);

            #post_syscall
        }
    }
}

fn generate_syscall_raw_decl(syscall: &Syscall) -> TokenStream2 {
    let raw_syscall_inputs = generate_raw_syscall_inputs(&syscall);
    let forward_args = generate_raw_through_calls(&syscall);

    let uninit_ident = format_ident!("uninit_{}", &syscall.name);
    let raw_ident = format_ident!("raw_{}", &syscall.name);

    let return_type = syscall.return_type();

    quote! {
        pub fn #raw_ident(#(#raw_syscall_inputs,)*) -> #return_type {
            #uninit_ident(#(#forward_args,)*)
        }
    }
}

pub fn generate_syscall_decls(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let yaml_path = std::path::PathBuf::from(manifest_dir).join(&file_path);

    let content = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", yaml_path.display()));

    let spec: SyscallSpec = serde_yaml::from_str(&content).unwrap();

    let decls = spec.syscalls.iter().map(|func| {
        let uninit = generate_syscall_uninit_decl(&func);
        let raw = generate_syscall_raw_decl(&func);
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
        #name = #num as u64
    }
}

pub fn generate_syscall_numbers(input: TokenStream) -> TokenStream {
    let file_path = syn::parse_macro_input!(input as syn::LitStr).value();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let yaml_path = std::path::PathBuf::from(manifest_dir).join(&file_path);

    let content = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", yaml_path.display()));

    let spec: SyscallSpec = serde_yaml::from_str(&content).unwrap();

    let lines = spec.syscalls.iter().map(|func| {
        generate_syscall_no_line(&func)
    });

    quote! {
        #[repr(u64)]
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

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    let yaml_path = std::path::PathBuf::from(manifest_dir).join(&file_path);

    let content = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", yaml_path.display()));

    let spec: SyscallSpec = serde_yaml::from_str(&content).unwrap();

    let lines = spec.errors.iter().map(|func| {
        generate_syscall_error_code_line(&func)
    });

    quote! {
        #[repr(i32)]
        #[derive(Eq,PartialEq,Copy,Debug,Clone)]
        pub enum SyscallErrorCode {
            #(#lines,)*
        }
    }
    .into()
}
