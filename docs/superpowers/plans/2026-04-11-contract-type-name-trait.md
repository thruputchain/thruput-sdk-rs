# `ContractTypeName` Trait Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple the spec name of a contract type from its Rust ident via a `ContractTypeName` trait, with optional `name = "..."` override on `#[contracttype]` and `#[contract_error]`.

**Architecture:** Add `ContractTypeName` trait to `groundhog-contract-spec`. The `#[contracttype]` and `#[contract_error]` attribute macros generate `impl ContractTypeName` (using the ident or custom name). The `#[derive(ContractTypeSpec)]` derive uses `<Self as ContractTypeName>::NAME` in TYPE_DEF instead of `stringify!(ident)`. Standalone derive users must impl `ContractTypeName` themselves.

**Tech Stack:** Rust proc macros (syn, quote, darling), groundhog-contract-spec

---

### Task 1: Add `ContractTypeName` trait

**Files:**
- Modify: `groundhog-contract-spec/src/lib.rs`
- Modify: `src/lib.rs` (re-export)

- [ ] **Step 1: Add the trait**

In `groundhog-contract-spec/src/lib.rs`, add after the `ContractTypeSpec` trait definition (after line 24):

```rust
pub trait ContractTypeName {
    const NAME: &'static str;
}
```

- [ ] **Step 2: Re-export from SDK**

In `src/lib.rs`, the trait is already accessible via `sdk::__contract_spec::ContractTypeName` (since `groundhog_contract_spec` is re-exported as `__contract_spec`). No additional re-export needed for macro-generated code.

However, add a public re-export for user-facing access. Add near line 121:

```rust
pub use groundhog_contract_spec::ContractTypeName;
```

- [ ] **Step 3: Verify compiles**

Run:
```bash
cargo check -p groundhog-contract-spec && cargo check -p sdk --features std
```

- [ ] **Step 4: Commit**

```bash
git add groundhog-contract-spec/src/lib.rs src/lib.rs
git commit -m "Add ContractTypeName trait to groundhog-contract-spec"
```

---

### Task 2: Update `#[derive(ContractTypeSpec)]` to use `ContractTypeName::NAME`

**Files:**
- Modify: `macros/src/derive_contract_type_spec.rs`

- [ ] **Step 1: Replace `stringify!(#ident)` with trait const**

In `derive_contract_type_spec.rs`, there are three places where `stringify!(#ident)` is used as the name in TYPE_DEF generation. Replace all three with `<Self as ::sdk::__contract_spec::ContractTypeName>::NAME`.

In `struct_type_def_tokens` (line 55), change:
```rust
                name: stringify!(#ident),
```
to:
```rust
                name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
```

In `enum_type_def_tokens`, the plain enum branch (line 99), change:
```rust
                    name: stringify!(#ident),
```
to:
```rust
                    name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
```

In `enum_type_def_tokens`, the union branch (line 125), change:
```rust
                name: stringify!(#ident),
```
to:
```rust
                name: <Self as ::sdk::__contract_spec::ContractTypeName>::NAME,
```

The `struct_type_def_tokens` function no longer needs the `ident` parameter for the name (it uses `Self`), but it's still used in the function signature by callers. Leave the parameter for now — removing it would be churn.

- [ ] **Step 2: Verify macro crate compiles**

Run:
```bash
cargo check -p groundhog-macros
```

- [ ] **Step 3: Commit**

```bash
git add macros/src/derive_contract_type_spec.rs
git commit -m "Use ContractTypeName::NAME in derive(ContractTypeSpec) TYPE_DEF"
```

---

### Task 3: Update `#[contracttype]` to generate `impl ContractTypeName`

**Files:**
- Modify: `macros/src/contract_type.rs`
- Modify: `macros/src/lib.rs` (parse args)

- [ ] **Step 1: Add arg parsing to `contract_type.rs`**

Replace the entire content of `macros/src/contract_type.rs`:

```rust
use darling::FromMeta;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Item;

#[derive(Debug, Default, FromMeta)]
pub struct ContractTypeArgs {
    pub name: Option<String>,
}

pub fn expand(args: ContractTypeArgs, item: Item) -> TokenStream {
    match item {
        Item::Enum(item) => {
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            quote! {
                #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #item

                #name
            }
        }
        Item::Struct(item) => {
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            quote! {
                #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #item

                #name
            }
        }
        other => syn::Error::new_spanned(
            other,
            "#[contracttype] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

fn expand_contract_type_name(
    ident: &syn::Ident,
    args: &ContractTypeArgs,
) -> TokenStream {
    let name_value = match &args.name {
        Some(custom) => quote! { #custom },
        None => quote! { stringify!(#ident) },
    };
    quote! {
        impl ::sdk::__contract_spec::ContractTypeName for #ident {
            const NAME: &'static str = #name_value;
        }
    }
}
```

- [ ] **Step 2: Update `lib.rs` to parse args**

In `macros/src/lib.rs`, replace the `contracttype` function (lines 124-131):

```rust
#[proc_macro_attribute]
pub fn contracttype(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let item = syn::parse_macro_input!(item_tokens as syn::Item);
    let args = if attr_args.is_empty() {
        contract_type::ContractTypeArgs::default()
    } else {
        let meta_list = match darling::ast::NestedMeta::parse_meta_list(
            attr_args.into(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return TokenStream::from(
                    darling::Error::from(e).write_errors(),
                )
            }
        };
        match contract_type::ContractTypeArgs::from_list(&meta_list) {
            Ok(v) => v,
            Err(e) => return TokenStream::from(e.write_errors()),
        }
    };
    contract_type::expand(args, item).into()
}
```

Add this import at the top of `lib.rs` (it's already used by `derive_exn.rs` via `darling::FromMeta` but the `from_list` call needs it in scope):

```rust
use darling::FromMeta;
```

Actually, `darling::FromMeta` is used inside the modules, not at the `lib.rs` level. Just use the fully qualified path `darling::ast::NestedMeta` and `contract_type::ContractTypeArgs::from_list` as shown above — no new imports needed in `lib.rs`.

- [ ] **Step 3: Verify compiles**

Run:
```bash
cargo check -p groundhog-macros
```

- [ ] **Step 4: Verify example contracts still compile**

Run:
```bash
cd /Users/geoff/groundhog/sdk_rs/example-contracts && cargo check -p spec_named_types --features dev
```

- [ ] **Step 5: Commit**

```bash
git add macros/src/contract_type.rs macros/src/lib.rs
git commit -m "Add name arg to #[contracttype] and generate impl ContractTypeName"
```

---

### Task 4: Update `#[contract_error]` to generate `impl ContractTypeName`

**Files:**
- Modify: `macros/src/derive_exn.rs` (add `name` to `ExnArgs`)
- Modify: `macros/src/contract_error.rs` (generate `impl ContractTypeName`)

- [ ] **Step 1: Add `name` field to `ExnArgs`**

In `macros/src/derive_exn.rs`, add `name` to the struct (line 7-9):

```rust
#[derive(Debug, Default, FromMeta)]
pub struct ExnArgs {
    pub version: Option<String>,
    pub name: Option<String>,
}
```

- [ ] **Step 2: Generate `impl ContractTypeName` in contract_error.rs**

In `macros/src/contract_error.rs`, add a helper function:

```rust
fn expand_contract_type_name(
    ident: &syn::Ident,
    args: &ExnArgs,
) -> TokenStream {
    let name_value = match &args.name {
        Some(custom) => quote! { #custom },
        None => quote! { stringify!(#ident) },
    };
    quote! {
        impl ::sdk::__contract_spec::ContractTypeName for #ident {
            const NAME: &'static str = #name_value;
        }
    }
}
```

Update `expand_enum` to generate the name impl. Add after the `alloc_marker_impl` binding:

```rust
    let name_impl = expand_contract_type_name(ident, &args);
```

And include it in the output:

```rust
    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #[repr(i32)]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl

        #name_impl
    }
```

Apply the same change to `expand_struct`:

```rust
    let name_impl = expand_contract_type_name(ident, &args);
```

```rust
    quote! {
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #item

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }

        #alloc_marker_impl

        #name_impl
    }
```

- [ ] **Step 3: Verify compiles and tests pass**

Run:
```bash
cargo check -p groundhog-macros && cargo test -p groundhog-macros --lib
```

- [ ] **Step 4: Verify example contracts still compile**

Run:
```bash
cd /Users/geoff/groundhog/sdk_rs/example-contracts && cargo check -p exceptions --features dev && cargo check -p multi_throws --features dev
```

- [ ] **Step 5: Update compile-fail stderr if needed**

Run:
```bash
cargo test --test compile_fail 2>&1 | tail -5
```

If the `contract_error_missing_discriminant` test fails (because the error output changed), regenerate:
```bash
TRYBUILD=overwrite cargo test --test compile_fail
```

- [ ] **Step 6: Commit**

```bash
git add macros/src/derive_exn.rs macros/src/contract_error.rs tests/compile-fail/contract_error_missing_discriminant.stderr
git commit -m "Add name arg to #[contract_error] and generate impl ContractTypeName"
```

---

### Task 5: Add compile-pass test for custom name

**Files:**
- Create: `tests/compile-pass/contracttype_custom_name.rs`

- [ ] **Step 1: Create the test**

Create `tests/compile-pass/contracttype_custom_name.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

#[contracttype(name = "SpecPayload")]
pub struct Payload {
    pub count: u32,
}

#[contracttype(name = "SpecChoice")]
pub enum Choice {
    Left = 1,
    Right = 2,
}

// Verify the names are what we set
const _: () = {
    assert!(
        matches!(
            <Payload as contract_spec::ContractTypeSpec>::TYPE_DEF,
            contract_spec::ContractTypeDefView::Struct(def) if {
                let name = def.name;
                name.len() == "SpecPayload".len()
                    && name.as_bytes()[0] == b'S'
                    && name.as_bytes()[4] == b'P'
            }
        )
    );
};

fn main() {}
```

Note: const assert with string equality is limited — we check length and a couple bytes. A simpler approach: just verify it compiles and the trait is satisfied. The actual name correctness is verified by the macro unit test in Task 6.

Actually, simplify the test to just verify compilation with the `name` arg:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

#[contracttype(name = "SpecPayload")]
pub struct Payload {
    pub count: u32,
}

#[contracttype(name = "SpecChoice")]
pub enum Choice {
    Left = 1,
    Right = 2,
}

fn main() {}
```

- [ ] **Step 2: Verify it passes**

Run:
```bash
cargo test --test compile_pass
```

- [ ] **Step 3: Commit**

```bash
git add tests/compile-pass/contracttype_custom_name.rs
git commit -m "Add compile-pass test for #[contracttype(name = \"...\")]"
```
