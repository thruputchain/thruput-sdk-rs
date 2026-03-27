# Error Handling Ergonomics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `or_throw()` and `or_re_throw()` extension methods on `Result`, re-export `thiserror` via `__private` to eliminate a transitive contract dependency, and extend `#[contract_error]` to support structs.

**Architecture:** Three independent features touching the SDK core (`src/`), proc macros (`macros/`), and code generator (`gen/`). The `OrThrow` and `ReThrow` traits live in the SDK crate; the code generator emits `impl ReThrow` for each generated client error enum. The `__private` module provides macro plumbing without polluting the public API.

**Tech Stack:** Rust, proc-macro2/quote/syn, thiserror

---

### Task 1: Add `OrThrow` trait to `src/throw.rs`

**Files:**
- Modify: `src/throw.rs:1-8`

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)]` module at the bottom of `src/throw.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::exn::Exn;
    use crate::xdr::*;

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
    #[repr(i32)]
    enum TestExn {
        Boom = 1,
    }
    impl Exn for TestExn {
        const SELECTOR: u64 = 0xDEAD;
    }

    #[test]
    fn or_throw_ok_returns_value() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.or_throw(TestExn::Boom), 42);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib -- throw::tests::or_throw_ok_returns_value`
Expected: FAIL — `or_throw` method not found on `Result`

- [ ] **Step 3: Implement `OrThrow` trait**

Add to `src/throw.rs` above the existing `throw` function:

```rust
/// Extension trait for replacing an error with a contract exception.
pub trait OrThrow<T> {
    /// On `Err`, throw `exn` as a contract exception. On `Ok`, return the value.
    fn or_throw<E: crate::Exn>(self, exn: E) -> T;
}

impl<T, Err> OrThrow<T> for Result<T, Err> {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn or_throw<E: crate::Exn>(self, exn: E) -> T {
        match self {
            Ok(v) => v,
            Err(_) => throw(exn),
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib -- throw::tests::or_throw_ok_returns_value`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/throw.rs
git commit -m "feat: add OrThrow extension trait on Result"
```

---

### Task 2: Add `ReThrow` trait and `OrReThrow` in `src/re_throw.rs`

**Files:**
- Create: `src/re_throw.rs`
- Modify: `src/lib.rs:34` (add `mod re_throw;`)
- Modify: `src/lib.rs:85` (add `pub use re_throw::*;`)

- [ ] **Step 1: Create `src/re_throw.rs` with traits**

```rust
/// A type that can re-throw itself as a contract exception or fault.
pub trait ReThrow {
    /// Re-throw this error. Exceptions are propagated as exceptions;
    /// faults panic (a fault stays a fault).
    fn re_throw(self) -> !;
}

/// Extension trait for transparently propagating a contract error.
pub trait OrReThrow<T> {
    /// On `Err`, re-throw the error. On `Ok`, return the value.
    fn or_re_throw(self) -> T;
}

impl<T, E: ReThrow> OrReThrow<T> for Result<T, E> {
    fn or_re_throw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => e.re_throw(),
        }
    }
}
```

- [ ] **Step 2: Wire up in `src/lib.rs`**

Add `mod re_throw;` after line 34 (after `mod throw;`):

```rust
mod throw;
mod re_throw;
```

Add `pub use re_throw::*;` after line 85 (after `pub use throw::*;`):

```rust
pub use throw::*;
pub use re_throw::*;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo test --lib -- --list 2>&1 | head -5`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src/re_throw.rs src/lib.rs
git commit -m "feat: add ReThrow and OrReThrow traits"
```

---

### Task 3: Implement `ReThrow` for `RawCallError` and `RawException`

**Files:**
- Modify: `src/call/call_result.rs:1-69`
- Modify: `src/exn/raw_error.rs:1-82`

- [ ] **Step 1: Write the failing test**

Add to the bottom of `src/call/call_result.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::re_throw::OrReThrow;

    #[test]
    fn or_re_throw_ok_returns_value() {
        let result: Result<i32, RawCallError> = Ok(42);
        assert_eq!(result.or_re_throw(), 42);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib -- call::call_result::tests::or_re_throw_ok_returns_value`
Expected: FAIL — `ReThrow` is not implemented for `RawCallError`

- [ ] **Step 3: Implement `ReThrow` for `RawException`**

Add to `src/exn/raw_error.rs` after the `PartialEq` impl (after line 81):

```rust
impl crate::re_throw::ReThrow for RawException {
    fn re_throw(self) -> ! {
        // Delegate to the existing inherent re_throw(&self) method
        RawException::re_throw(&self)
    }
}
```

- [ ] **Step 4: Implement `ReThrow` for `RawCallError`**

Add to `src/call/call_result.rs` after the `Display` impl (after line 33):

```rust
impl crate::re_throw::ReThrow for RawCallError {
    fn re_throw(self) -> ! {
        match self {
            RawCallError::Exception(exn) => {
                crate::re_throw::ReThrow::re_throw(exn)
            }
            RawCallError::Fault(f) => panic!("{f}"),
        }
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib -- call::call_result::tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/call/call_result.rs src/exn/raw_error.rs
git commit -m "feat: impl ReThrow for RawCallError and RawException"
```

---

### Task 4: Add `__private` module and re-export thiserror

**Files:**
- Modify: `src/lib.rs:27-28`

- [ ] **Step 1: Add `__private` module**

Add after the `#[doc(hidden)] pub mod internal;` line (after line 28 of `src/lib.rs`):

```rust
#[doc(hidden)]
pub mod internal;

#[doc(hidden)]
pub mod __private {
    pub use ::thiserror;
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo test --lib -- --list 2>&1 | head -5`
Expected: compiles without errors

- [ ] **Step 3: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add __private module with thiserror re-export"
```

---

### Task 5: Update `#[contract_error]` macro to use `__private` thiserror path

**Files:**
- Modify: `macros/src/contract_error.rs:55-57`

- [ ] **Step 1: Change thiserror derive path**

In `macros/src/contract_error.rs`, change line 57 from:

```rust
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, thiserror::Error)]
```

to:

```rust
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
```

- [ ] **Step 2: Verify example contracts compile**

Run: `cargo check -p counter`
Expected: compiles without errors (counter uses `#[contract_error]`)

- [ ] **Step 3: Remove `thiserror` from example contract `Cargo.toml` files**

Remove the `thiserror` dependency line from each of these files:
- `example-contracts/counter/Cargo.toml`
- `example-contracts/caller/Cargo.toml`
- `example-contracts/erc20/Cargo.toml`
- `example-contracts/erc20_low_level/Cargo.toml`
- `example-contracts/erc20_no_storage/Cargo.toml`
- `example-contracts/exceptions/Cargo.toml`
- `example-contracts/swap/Cargo.toml`
- `example-contracts/multi_throws/Cargo.toml`
- `example-contracts/z_deprecated_playground/Cargo.toml`

- [ ] **Step 4: Verify examples still compile without thiserror**

Run: `cargo check -p counter -p caller -p erc20 -p exceptions -p multi_throws`
Expected: compiles — thiserror is now provided by the SDK's `__private` re-export

- [ ] **Step 5: Commit**

```bash
git add macros/src/contract_error.rs example-contracts/*/Cargo.toml
git commit -m "feat: route thiserror through sdk::__private, remove from contract crates"
```

---

### Task 6: Extend `#[contract_error]` to support structs

**Files:**
- Modify: `macros/src/contract_error.rs:1-77`

- [ ] **Step 1: Write a compile-test for struct contract_error**

Add a unit struct error type in `example-contracts/exceptions/src/implementation.rs` (this contract already has exception types). Add after the existing error enums:

```rust
#[contract_error]
#[error("not authorized")]
pub struct Unauthorized;
```

- [ ] **Step 2: Verify it fails to compile**

Run: `cargo check -p exceptions`
Expected: FAIL — `#[contract_error] is only supported on enums`

- [ ] **Step 3: Refactor the macro to handle both enums and structs**

Replace the entire contents of `macros/src/contract_error.rs` with:

```rust
use super::derive_exn::ExnArgs;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Item, ItemEnum, ItemStruct};

/// Expand `#[contract_error]` or `#[contract_error(version = "...")]`
/// into the full derive set + `impl Exn`.
///
/// For enums, also adds `#[repr(i32)]`.
///
/// With `feature = "dev"` (in the consuming crate):
///   - Derives `thiserror::Error` for Display/Error impls
///   - Keeps `#[error("...")]` attributes
///
/// Without `dev`:
///   - Does NOT derive `thiserror::Error`
///   - Strips `#[error("...")]` attributes so they don't cause errors
///   - Error strings are completely absent from the wasm binary
pub fn expand(args: ExnArgs, item: Item) -> TokenStream {
    match item {
        Item::Enum(e) => expand_enum(args, e),
        Item::Struct(s) => expand_struct(args, s),
        other => syn::Error::new_spanned(
            other,
            "#[contract_error] is only supported on enums and structs",
        )
        .into_compile_error(),
    }
}

fn expand_enum(args: ExnArgs, item: ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());

    let mut item_stripped = item.clone();
    strip_enum_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
        #[repr(i32)]
        #item

        #[cfg(not(feature = "dev"))]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #[repr(i32)]
        #item_stripped

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }
    }
}

fn expand_struct(args: ExnArgs, item: ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let selector = compute_selector(&args, &ident.to_string());

    let mut item_stripped = item.clone();
    strip_struct_error_attrs(&mut item_stripped);

    quote! {
        #[cfg(feature = "dev")]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
        #item

        #[cfg(not(feature = "dev"))]
        #[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR)]
        #item_stripped

        impl ::sdk::exn::Exn for #ident {
            const SELECTOR: u64 = #selector;
        }
    }
}

fn compute_selector(args: &ExnArgs, type_name: &str) -> u64 {
    let version = args.version.clone().unwrap_or_else(|| {
        std::env::var("CARGO_PKG_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string())
    });
    let crate_name = std::env::var("CARGO_PKG_NAME")
        .unwrap_or_else(|_| "_unknown_".to_string());

    groundhog_contract_util::error_selector(&version, &crate_name, type_name)
}

/// Remove all `#[error(...)]` attributes from an enum's variants.
fn strip_enum_error_attrs(item: &mut ItemEnum) {
    for variant in &mut item.variants {
        variant.attrs.retain(|attr| !attr.path().is_ident("error"));
    }
}

/// Remove `#[error(...)]` from the struct itself and
/// `#[source]`/`#[from]` from its fields (thiserror attributes).
fn strip_struct_error_attrs(item: &mut ItemStruct) {
    item.attrs.retain(|attr| !attr.path().is_ident("error"));
    if let syn::Fields::Named(ref mut fields) = item.fields {
        for field in &mut fields.named {
            field.attrs.retain(|attr| {
                let name = attr.path().get_ident().map(|i| i.to_string());
                !matches!(name.as_deref(), Some("error" | "source" | "from"))
            });
        }
    }
    if let syn::Fields::Unnamed(ref mut fields) = item.fields {
        for field in &mut fields.unnamed {
            field.attrs.retain(|attr| {
                let name = attr.path().get_ident().map(|i| i.to_string());
                !matches!(name.as_deref(), Some("error" | "source" | "from"))
            });
        }
    }
}
```

- [ ] **Step 4: Verify struct contract_error compiles**

Run: `cargo check -p exceptions`
Expected: PASS — the `Unauthorized` struct now compiles

- [ ] **Step 5: Verify existing enum contract_errors still work**

Run: `cargo test --lib`
Expected: all 14 tests pass

- [ ] **Step 6: Commit**

```bash
git add macros/src/contract_error.rs example-contracts/exceptions/src/implementation.rs
git commit -m "feat: extend #[contract_error] to support structs"
```

---

### Task 7: Generate `impl ReThrow` for client error enums

**Files:**
- Modify: `gen/src/client.rs:257-312` (in `generate_error_enum`)
- Modify: `gen/src/client.rs:207-229` (in `generate_client_fn`, the non-try method)

- [ ] **Step 1: Add `ReThrow` impl to generated error enum**

In `gen/src/client.rs`, in the `generate_error_enum` function, add the `ReThrow` impl inside the `quote!` block after the `Display` impl (after the closing `}` of the `Display` impl):

```rust
impl ::sdk::re_throw::ReThrow for #error_enum_ident {
    fn re_throw(self) -> ! {
        match self {
            #(Self::#throws_list(e) => ::sdk::throw(e),)*
            Self::Library(e) => ::sdk::throw(e),
            Self::Unknown(raw) => ::sdk::re_throw::ReThrow::re_throw(raw),
            Self::Fault(f) => panic!("{f}"),
        }
    }
}
```

The `#throws_list` iteration uses the same `throws_list: &[&syn::Ident]` already in scope. Each typed variant calls `::sdk::throw(e)` because the inner type implements `Exn`. `Unknown` uses the fully-qualified `ReThrow::re_throw` to disambiguate from the inherent method on `RawException`.

To produce the match arms, build them the same way as `display_declared_arms`:

```rust
let rethrow_declared_arms: Vec<TokenStream> = throws_list
    .iter()
    .map(|ident| {
        quote! { Self::#ident(e) => ::sdk::throw(e) }
    })
    .collect();
```

Then emit:

```rust
impl ::sdk::re_throw::ReThrow for #error_enum_ident {
    fn re_throw(self) -> ! {
        match self {
            #(#rethrow_declared_arms,)*
            Self::Library(e) => ::sdk::throw(e),
            Self::Unknown(raw) => ::sdk::re_throw::ReThrow::re_throw(raw),
            Self::Fault(f) => panic!("{f}"),
        }
    }
}
```

- [ ] **Step 2: Simplify non-try method to use `or_re_throw()`**

In `gen/src/client.rs`, in `generate_client_fn`, replace the match block for the non-try method (lines 218-228):

```rust
// Before:
with_call_tracking!(#call_info, {
    match ::sdk::call::try_call_raw::<_, #return_type>(&self.address, entrypoint, self.config.gas_limit, &args) {
        Ok(val) => val,
        Err(::sdk::call::RawCallError::Exception(ref exn)) => {
            exn.re_throw()
        }
        Err(::sdk::call::RawCallError::Fault(f)) => {
            panic!("{}", f)
        }
    }
})

// After:
with_call_tracking!(#call_info, {
    ::sdk::call::try_call_raw::<_, #return_type>(&self.address, entrypoint, self.config.gas_limit, &args)
        .or_re_throw()
})
```

This requires `OrReThrow` to be in scope. Since the generated code is inside a `const _: () = { ... }` block that already has `use ::sdk::...` imports, add:

```rust
use ::sdk::re_throw::OrReThrow;
```

to the existing `use` block inside the `const _: ()` (around line 55 in the generated output).

- [ ] **Step 3: Verify an example contract with a client compiles**

Run: `cargo check -p caller`
Expected: compiles — the generated client uses `or_re_throw()` and implements `ReThrow`

- [ ] **Step 4: Run full test suite**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add gen/src/client.rs
git commit -m "feat: generate ReThrow impl for client errors, use or_re_throw() in clients"
```

---

### Task 8: Final verification and cleanup

**Files:**
- No new files

- [ ] **Step 1: Run full SDK test suite**

Run: `cargo test --lib`
Expected: all tests pass

- [ ] **Step 2: Check all example contracts compile**

Run: `cargo check -p counter -p caller -p erc20 -p erc20_low_level -p erc20_no_storage -p exceptions -p swap -p multi_throws`
Expected: all compile without errors

- [ ] **Step 3: Run `cargo fmt`**

Run: `cargo fmt --all`

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --lib -- -D warnings`

- [ ] **Step 5: Fix any issues, then commit**

```bash
git add -A
git commit -m "chore: fmt + clippy fixes"
```
