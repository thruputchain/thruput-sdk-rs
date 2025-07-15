use proc_macro::TokenStream;

mod contract;
mod static_key;
mod storage;
mod syscalls;

#[proc_macro]
pub fn static_key(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::LitStr);
    static_key::static_key_output(input)
}

/// Generate contract-related code
#[proc_macro_attribute]
pub fn contract(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    contract::contract_output(attr_args, item_tokens)
}

#[proc_macro_derive(Storage)]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    storage::generate(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
pub fn generate_syscall_decls(input: TokenStream) -> TokenStream {
    syscalls::generate_syscall_decls(input)
}

#[proc_macro]
pub fn generate_syscall_numbers(input: TokenStream) -> TokenStream {
    syscalls::generate_syscall_numbers(input)
}

#[proc_macro]
pub fn generate_syscall_error_codes(input: TokenStream) -> TokenStream {
    syscalls::generate_syscall_error_codes(input)
}

#[proc_macro]
pub fn generate_syscall_error_helper(input: TokenStream) -> TokenStream {
    syscalls::generate_syscall_error_helper(input)
}
