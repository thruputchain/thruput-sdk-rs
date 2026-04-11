# `#[contracttype]` Macro Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `#[contracttype]` attribute macro and `#[derive(ContractTypeSpec)]` derive macro so contract authors can annotate custom types instead of hand-writing spec impls.

**Architecture:** Extract the `ContractTypeSpec` codegen from `contract_error.rs` into a new `derive_contract_type_spec.rs` derive macro. Add a thin `#[contracttype]` attribute macro. Refactor `#[contract_error]` to delegate to the new derive. Migrate the `spec_named_types` example.

**Tech Stack:** Rust proc macros (syn, quote, proc_macro2), groundhog-codegen

---

### Task 1: Demonstrate what breaks without `ContractTypeSpec`

Create a minimal example contract that uses a bare struct in a method signature, showing the compile error.

**Files:**
- Create: `example-contracts/bare_type_fails/Cargo.toml`
- Create: `example-contracts/bare_type_fails/src/lib.rs`
- Modify: `example-contracts/Cargo.toml` (add workspace member)

- [ ] **Step 1: Create the failing contract crate**

`example-contracts/bare_type_fails/Cargo.toml`:
```toml
[package]
name = "bare_type_fails"
description = "Demonstrates compile failure when a type lacks ContractTypeSpec"
version.workspace = true
authors.workspace = true
license.workspace = true
publish.workspace = true
repository.workspace = true
homepage.workspace = true
keywords.workspace = true
categories.workspace = true
edition.workspace = true

[features]
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["cdylib", "rlib"]
doctest = false
test = false

[dependencies]
sdk = { workspace = true, features = ["alloc"] }

[dev-dependencies]
bare_type_fails = { path = ".", features = ["dev"] }
```

`example-contracts/bare_type_fails/src/lib.rs`:
```rust
#![no_std]

extern crate alloc;

use sdk::*;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[xdrs(crate = "::sdk::__xdrs")]
pub struct Payload {
    pub count: u32,
}

#[derive(Default)]
pub struct BareTypeContract;

#[contract(version = "0.1.0")]
impl BareTypeContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }
}
```

Add `"bare_type_fails"` to the workspace members list in `example-contracts/Cargo.toml`.

- [ ] **Step 2: Verify it fails to compile**

Run:
```bash
cd example-contracts && cargo check -p bare_type_fails --features dev 2>&1
```
Expected: compile error mentioning `ContractTypeRefSpec` is not satisfied for `Payload`.

- [ ] **Step 3: Commit**

```bash
git add example-contracts/bare_type_fails/ example-contracts/Cargo.toml
git commit -m "Add bare_type_fails example demonstrating missing ContractTypeSpec"
```

---

### Task 2: Create `#[derive(ContractTypeSpec)]` derive macro

Move the shared codegen out of `contract_error.rs` into a new derive macro.

**Files:**
- Create: `macros/src/derive_contract_type_spec.rs`
- Modify: `macros/src/lib.rs` (register derive)
- Modify: `src/lib.rs` (re-export derive)

- [ ] **Step 1: Create `derive_contract_type_spec.rs`**

This file contains the derive macro entry point and all the shared helpers moved from `contract_error.rs`. The derive handles structs and enums (both plain and data-carrying).

`macros/src/derive_contract_type_spec.rs`:
```rust
use groundhog_codegen as codegen;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand(input: &DeriveInput) -> TokenStream {
    match &input.data {
        syn::Data::Struct(data) => expand_struct(input, data),
        syn::Data::Enum(data) => expand_enum(input, data),
        syn::Data::Union(_) => syn::Error::new_spanned(
            input,
            "#[derive(ContractTypeSpec)] is not supported on unions",
        )
        .into_compile_error(),
    }
}

fn expand_struct(input: &DeriveInput, data: &syn::DataStruct) -> TokenStream {
    let ident = &input.ident;
    let type_def = match struct_type_def_tokens(ident, &data.fields) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let type_defs = match struct_type_defs_tokens(&data.fields) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let raw_typedef = emit_raw_typedef_fragment(ident);

    quote! {
        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #raw_typedef
    }
}

fn expand_enum(input: &DeriveInput, data: &syn::DataEnum) -> TokenStream {
    let ident = &input.ident;
    let type_def = match enum_type_def_tokens(ident, &data.variants) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let type_defs = match enum_type_defs_tokens(&data.variants) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };
    let raw_typedef = emit_raw_typedef_fragment(ident);

    quote! {
        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #raw_typedef
    }
}

fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    codegen::spec_reg::emit_raw_typedef_fragment_stream_tokens_with_cfg(
        quote! { target_family = "wasm" },
        quote! { <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEFS },
    )
}

pub fn struct_type_def_tokens(
    ident: &syn::Ident,
    fields: &syn::Fields,
) -> syn::Result<TokenStream> {
    let field_tokens = struct_field_tokens(fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefView::Struct(
            ::sdk::__contract_spec::ContractStructDefView {
                name: stringify!(#ident),
                fields: &[#(#field_tokens),*],
            }
        )
    })
}

pub fn struct_type_defs_tokens(
    fields: &syn::Fields,
) -> syn::Result<TokenStream> {
    let nested = fields_type_defs_tokens(fields)?;
    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            ::sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
            #(#nested),*
        ])
    })
}

pub fn enum_type_def_tokens(
    ident: &syn::Ident,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<TokenStream> {
    let discriminants = enum_discriminant_tokens(variants)?;
    let is_plain_enum = variants
        .iter()
        .all(|variant| matches!(variant.fields, syn::Fields::Unit));

    if is_plain_enum {
        let variant_tokens = variants
            .iter()
            .zip(discriminants.iter())
            .map(|(variant, discriminant)| {
                let name = variant.ident.to_string();
                quote! {
                    ::sdk::__contract_spec::ContractEnumVariantView {
                        name: #name,
                        value: #discriminant,
                    }
                }
            })
            .collect::<Vec<_>>();

        return Ok(quote! {
            ::sdk::__contract_spec::ContractTypeDefView::Enum(
                ::sdk::__contract_spec::ContractEnumDefView {
                    name: stringify!(#ident),
                    variants: &[#(#variant_tokens),*],
                }
            )
        });
    }

    let cases = variants
        .iter()
        .zip(discriminants.iter())
        .map(|(variant, discriminant)| {
            let name = variant.ident.to_string();
            let case_type = enum_variant_case_type_tokens(variant)?;
            Ok(quote! {
                ::sdk::__contract_spec::ContractUnionCaseView {
                    name: #name,
                    discriminant: #discriminant,
                    case_type: #case_type,
                }
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefView::Union(
            ::sdk::__contract_spec::ContractUnionDefView {
                name: stringify!(#ident),
                cases: &[#(#cases),*],
            }
        )
    })
}

pub fn enum_type_defs_tokens(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<TokenStream> {
    let nested = variants
        .iter()
        .map(enum_variant_type_defs_tokens)
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        ::sdk::__contract_spec::ContractTypeDefsView::Many(&[
            ::sdk::__contract_spec::ContractTypeDefsView::One(Self::TYPE_DEF),
            #(#nested),*
        ])
    })
}

pub fn enum_discriminant_tokens(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<TokenStream>> {
    variants
        .iter()
        .map(|variant| {
            let Some((_, expr)) = &variant.discriminant else {
                return Err(syn::Error::new_spanned(
                    variant,
                    "enum variants must use explicit discriminants for contract specs",
                ));
            };
            Ok(quote! { #expr })
        })
        .collect()
}

fn struct_field_tokens(fields: &syn::Fields) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let name = field.ident.as_ref().unwrap().to_string();
                let ty =
                    codegen::contract::contract_type_view_tokens(&field.ty)?;
                Ok(quote! {
                    ::sdk::__contract_spec::ContractFieldView {
                        name: #name,
                        ty: #ty,
                    }
                })
            })
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let name = format!("_{index}");
                let ty =
                    codegen::contract::contract_type_view_tokens(&field.ty)?;
                Ok(quote! {
                    ::sdk::__contract_spec::ContractFieldView {
                        name: #name,
                        ty: #ty,
                    }
                })
            })
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn fields_type_defs_tokens(
    fields: &syn::Fields,
) -> syn::Result<Vec<TokenStream>> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                codegen::contract::contract_type_typedef_tokens(&field.ty)
            })
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .map(|field| {
                codegen::contract::contract_type_typedef_tokens(&field.ty)
            })
            .collect(),
        syn::Fields::Unit => Ok(vec![]),
    }
}

fn enum_variant_type_defs_tokens(
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    match &variant.fields {
        syn::Fields::Unit => {
            Ok(quote! { ::sdk::__contract_spec::ContractTypeDefsView::Empty })
        }
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            codegen::contract::contract_type_typedef_tokens(
                &fields.unnamed.first().unwrap().ty,
            )
        }
        syn::Fields::Named(fields) if fields.named.len() == 1 => {
            codegen::contract::contract_type_typedef_tokens(
                &fields.named.first().unwrap().ty,
            )
        }
        _ => Err(syn::Error::new_spanned(
            variant,
            "variants with multiple payload fields are not yet supported in contract specs",
        )),
    }
}

fn enum_variant_case_type_tokens(
    variant: &syn::Variant,
) -> syn::Result<TokenStream> {
    match &variant.fields {
        syn::Fields::Unit => Ok(quote! { None }),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = codegen::contract::contract_type_view_tokens(
                &fields.unnamed.first().unwrap().ty,
            )?;
            Ok(quote! { Some(#ty) })
        }
        syn::Fields::Named(fields) if fields.named.len() == 1 => {
            let ty = codegen::contract::contract_type_view_tokens(
                &fields.named.first().unwrap().ty,
            )?;
            Ok(quote! { Some(#ty) })
        }
        _ => Err(syn::Error::new_spanned(
            variant,
            "variants with multiple payload fields are not yet supported in contract specs",
        )),
    }
}
```

- [ ] **Step 2: Register the derive in `macros/src/lib.rs`**

Add at the top with other module declarations:
```rust
mod derive_contract_type_spec;
```

Add the derive macro function:
```rust
#[proc_macro_derive(ContractTypeSpec)]
pub fn derive_contract_type_spec(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    derive_contract_type_spec::expand(&input).into()
}
```

- [ ] **Step 3: Re-export from `src/lib.rs`**

Add alongside the other macro re-exports:
```rust
pub use groundhog_macros::ContractTypeSpec;
```

- [ ] **Step 4: Verify the macro crate compiles**

Run:
```bash
cargo check -p groundhog-macros
```
Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add macros/src/derive_contract_type_spec.rs macros/src/lib.rs src/lib.rs
git commit -m "Add #[derive(ContractTypeSpec)] derive macro"
```

---

### Task 3: Create `#[contracttype]` attribute macro

A thin wrapper that injects derives and repr.

**Files:**
- Create: `macros/src/contract_type.rs`
- Modify: `macros/src/lib.rs` (register attribute macro)
- Modify: `src/lib.rs` (re-export)

- [ ] **Step 1: Create `macros/src/contract_type.rs`**

```rust
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

pub fn expand(item: Item) -> TokenStream {
    match item {
        Item::Enum(item) => quote! {
            #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #[repr(i32)]
            #item
        },
        Item::Struct(item) => quote! {
            #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #item
        },
        other => syn::Error::new_spanned(
            other,
            "#[contracttype] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}
```

- [ ] **Step 2: Register in `macros/src/lib.rs`**

Add module declaration:
```rust
mod contract_type;
```

Add macro:
```rust
#[proc_macro_attribute]
pub fn contracttype(
    _attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    contract_type::expand(item).into()
}
```

- [ ] **Step 3: Re-export from `src/lib.rs`**

Add alongside other macro re-exports:
```rust
pub use groundhog_macros::contracttype;
```

- [ ] **Step 4: Verify compiles**

Run:
```bash
cargo check -p groundhog-macros && cargo check -p sdk --features std
```
Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add macros/src/contract_type.rs macros/src/lib.rs src/lib.rs
git commit -m "Add #[contracttype] attribute macro"
```

---

### Task 4: Fix the `bare_type_fails` example with `#[contracttype]`

**Files:**
- Modify: `example-contracts/bare_type_fails/src/lib.rs`

- [ ] **Step 1: Replace manual derives with `#[contracttype]`**

Update `example-contracts/bare_type_fails/src/lib.rs`:
```rust
#![no_std]

extern crate alloc;

use sdk::*;

#[contracttype]
pub struct Payload {
    pub count: u32,
}

#[derive(Default)]
pub struct BareTypeContract;

#[contract(version = "0.1.0")]
impl BareTypeContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cd example-contracts && cargo check -p bare_type_fails --features dev
```
Expected: compiles cleanly — `#[contracttype]` generates the `ContractTypeSpec` impl that satisfies `ContractTypeRefSpec`.

- [ ] **Step 3: Commit**

```bash
git add example-contracts/bare_type_fails/src/lib.rs
git commit -m "Fix bare_type_fails using #[contracttype]"
```

---

### Task 5: Refactor `#[contract_error]` to delegate to `#[derive(ContractTypeSpec)]`

**Files:**
- Modify: `macros/src/contract_error.rs`

- [ ] **Step 1: Replace inline ContractTypeSpec codegen with derive injection**

Rewrite `expand_enum` and `expand_struct` in `contract_error.rs` to inject `#[derive(ContractTypeSpec)]` instead of generating the `impl ContractTypeSpec` and raw typedef fragment inline. Remove all the now-unused helper functions that moved to `derive_contract_type_spec.rs`.

New `expand_enum`:
```rust
fn expand_enum(args: ExnArgs, item: syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let alloc_marker_impl = match enum_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl
    }
}
```

New `expand_struct`:
```rust
fn expand_struct(args: ExnArgs, item: syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());
    let alloc_marker_impl = match struct_alloc_enabled_impl_tokens(&item) {
        Ok(tokens) => tokens,
        Err(err) => return err.into_compile_error(),
    };

    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl
    }
}
```

Delete these functions from `contract_error.rs` (they now live in `derive_contract_type_spec.rs`):
- `emit_raw_typedef_fragment`
- `struct_type_def_tokens`
- `struct_type_defs_tokens`
- `enum_type_def_tokens`
- `enum_type_defs_tokens`
- `struct_field_tokens`
- `enum_variant_case_type_tokens`
- `enum_discriminant_tokens`
- `enum_variant_type_defs_tokens`
- `fields_type_defs_tokens`
- `field_types` (if only used by deleted functions)

Keep:
- `expand`, `expand_enum`, `expand_struct` (rewritten)
- `compute_selector`
- `enum_alloc_enabled_impl_tokens`, `struct_alloc_enabled_impl_tokens`, `alloc_enabled_impl_tokens`
- `enum_variant_field_types`, `field_types` (used by alloc impls)

- [ ] **Step 2: Run existing tests**

Run:
```bash
cargo test -p groundhog-macros --lib && cargo test -p groundhog-codegen --lib
```
Expected: all pass. The `contract_error::tests::enum_type_def_tokens_accepts_explicit_i32_min_discriminant` test references `enum_type_def_tokens` which moved — this test either needs to move to `derive_contract_type_spec` or be deleted if covered by the derive's integration behavior.

If that test fails because `enum_type_def_tokens` moved, move the test to `derive_contract_type_spec.rs` and adjust the call signature (now takes `(&Ident, &Punctuated<Variant, Token![,]>)` instead of `&ItemEnum`):
```rust
#[cfg(test)]
mod tests {
    use super::enum_type_def_tokens;

    #[test]
    fn enum_type_def_tokens_accepts_explicit_i32_min_discriminant() {
        let ident: syn::Ident = syn::parse_quote!(Error);
        let variants: syn::punctuated::Punctuated<syn::Variant, syn::Token![,]> =
            syn::parse_quote! {
                Min = -2147483647 - 1,
            };

        let tokens = enum_type_def_tokens(&ident, &variants).unwrap().to_string();
        assert!(tokens.contains("- 2147483647 - 1"));
    }
}
```

- [ ] **Step 3: Verify example contracts still compile**

Run:
```bash
cd example-contracts && cargo check -p exceptions --features dev && cargo check -p multi_throws --features dev && cargo check -p imported_error_type --features dev
```
Expected: all compile cleanly — `#[contract_error]` now injects `#[derive(ContractTypeSpec)]` which generates the same `impl ContractTypeSpec`.

- [ ] **Step 4: Commit**

```bash
git add macros/src/contract_error.rs macros/src/derive_contract_type_spec.rs
git commit -m "Refactor #[contract_error] to delegate to #[derive(ContractTypeSpec)]"
```

---

### Task 6: Migrate `spec_named_types` example to `#[contracttype]`

**Files:**
- Modify: `example-contracts/spec_named_types/src/implementation.rs`
- Modify: `example-contracts/spec_named_types/Cargo.toml` (update description)

- [ ] **Step 1: Replace hand-written impls with `#[contracttype]`**

Rewrite `example-contracts/spec_named_types/src/implementation.rs`:
```rust
extern crate alloc;

use alloc::vec::Vec;
use sdk::*;

#[contracttype]
pub struct Payload {
    pub count: u32,
}

#[contracttype]
pub enum Choice {
    Left = 1,
    Right = 2,
}

#[contracttype]
pub enum Envelope {
    Empty = 0,
    Wrapped { payload: Payload } = 1,
}

pub type PayloadPair = (Payload, Option<Payload>);
pub type Byte = u8;
pub type Blob = Vec<Byte>;
pub type Hash = [Byte; 32];
pub type BlobEnvelope = (Blob, Hash);

#[derive(Default)]
pub struct SpecNamedTypesContract;

#[contract(version = "0.3.0")]
impl SpecNamedTypesContract {
    pub fn echo(payload: Payload) -> Payload {
        payload
    }

    pub fn choose(choice: Choice) -> Choice {
        choice
    }

    pub fn wrap(flag: bool, payload: Payload) -> Envelope {
        if flag {
            Envelope::Wrapped { payload }
        } else {
            Envelope::Empty
        }
    }

    pub fn echo_pair(payloads: PayloadPair) -> PayloadPair {
        payloads
    }

    pub fn echo_blob(blob: Blob) -> Blob {
        blob
    }

    pub fn echo_hash(hash: Hash) -> Hash {
        hash
    }

    pub fn echo_blob_envelope(envelope: BlobEnvelope) -> BlobEnvelope {
        envelope
    }
}
```

This removes: all manual `ContractTypeSpec` impls, all manual `use` of spec view types, and the entire `raw_typedefs` module.

- [ ] **Step 2: Update description**

In `example-contracts/spec_named_types/Cargo.toml`, change description:
```toml
description = "A contract demonstrating #[contracttype] for named type spec generation"
```

- [ ] **Step 3: Verify it compiles**

Run:
```bash
cd example-contracts && cargo check -p spec_named_types --features dev
```
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add example-contracts/spec_named_types/
git commit -m "Migrate spec_named_types to #[contracttype]"
```

---

### Task 7: Clean up `bare_type_fails` example

Now that the macro works end-to-end, either remove this example (it served its demo purpose) or rename it to something useful.

**Files:**
- Remove: `example-contracts/bare_type_fails/` (directory)
- Modify: `example-contracts/Cargo.toml` (remove from workspace members)

- [ ] **Step 1: Remove the example and workspace entry**

Remove the `bare_type_fails` directory and its entry from `example-contracts/Cargo.toml` workspace members.

- [ ] **Step 2: Verify workspace compiles**

Run:
```bash
cd example-contracts && cargo check --workspace --features dev
```
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add -A example-contracts/bare_type_fails example-contracts/Cargo.toml
git commit -m "Remove bare_type_fails demo example"
```
