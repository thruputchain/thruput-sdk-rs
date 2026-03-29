# Auto-derive client name for bare impls — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `#[contract]` on a bare `impl Foo` auto-derive `FooClient` instead of requiring an explicit `client = FooClient` attribute.

**Architecture:** Change `ItemImpl::client_ident()` in the gen crate to derive `{SelfType}Client` for bare impls (no trait). Remove the compile error that required `client =` on bare impls. Simplify the macro entry point branching. Update the `codegen_impl_noclient` example to actually have no client attribute.

**Tech Stack:** Rust proc-macro, syn, quote

---

### Task 1: Update `ItemImpl::client_ident()` to auto-derive for bare impls

**Files:**
- Modify: `gen/src/contract.rs:185-192`

- [ ] **Step 1: Edit `ItemImpl::client_ident()`**

Replace the current implementation:

```rust
fn client_ident(&self) -> Option<syn::Ident> {
    // Always return None: when this impl is for a trait, the trait's
    // client_ident() is the authoritative source (and may carry a custom
    // name like `AdderProxy`).  Synthesising a default here would shadow
    // the trait's custom name via ConfiguredContract::client_ident()'s
    // fallback chain.
    None
}
```

With:

```rust
fn client_ident(&self) -> Option<syn::Ident> {
    if self.trait_.is_some() {
        // When implementing a trait, return None so the trait's
        // client_ident() is authoritative (may carry a custom name
        // like `AdderProxy`).
        return None;
    }
    // Bare impl: derive from Self type, e.g. Foo -> FooClient
    if let syn::Type::Path(tp) = &*self.self_ty {
        tp.path
            .segments
            .last()
            .map(|seg| format_ident!("{}Client", seg.ident))
    } else {
        None
    }
}
```

- [ ] **Step 2: Verify gen crate compiles**

Run: `cargo check -p groundhog-gen`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add gen/src/contract.rs
git commit -m "gen: auto-derive client_ident for bare impls"
```

---

### Task 2: Simplify macro entry point client logic

**Files:**
- Modify: `macros/src/contract.rs:111-129`

- [ ] **Step 1: Replace the client branching in the impl arm**

Replace the current logic:

```rust
if config.client_ident().is_some() {
    // It would be incorrect to generate a client interface here if the item is implementing a trait,
    // as the trait is already implementing a client interface.
    // It'd be too easy to double-generate the client struct.
    if item.trait_.is_some() {
        out.extend(quote::quote! {
            compile_error!("Adding an explicit client attribute to an impl of a trait is not allowed.  This would double-generate the client interface.  Add the client attribute to the trait instead.");
        });
        return Ok(out);
    }

    // Generate client interface
    out.extend(codegen::client::generate(item, config)?);
} else if item.trait_.is_none() {
    out.extend(quote::quote! {
        compile_error!("client = ClientName is required on an impl without a trait (no client interface would otherwise be generated)");
    });
    return Ok(out);
}
```

With:

```rust
if config.client_ident().is_some() && item.trait_.is_some() {
    out.extend(quote::quote! {
        compile_error!("Adding an explicit client attribute to an impl of a trait is not allowed.  This would double-generate the client interface.  Add the client attribute to the trait instead.");
    });
    return Ok(out);
}

if item.trait_.is_none() {
    // Bare impl: generate client (name from config or auto-derived)
    out.extend(codegen::client::generate(item, config)?);
}
```

- [ ] **Step 2: Verify macros crate compiles**

Run: `cargo check -p sdk-macros`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add macros/src/contract.rs
git commit -m "macros: simplify bare-impl client generation, remove mandatory client= error"
```

---

### Task 3: Update `codegen_impl_noclient` example to actually have no client attribute

**Files:**
- Modify: `example-contracts/codegen_impl_noclient/src/implementation.rs`
- Modify: `example-contracts/codegen_impl_noclient/Cargo.toml`

- [ ] **Step 1: Remove `client = CalcClient` from the contract attribute**

In `example-contracts/codegen_impl_noclient/src/implementation.rs`, change:

```rust
#[contract(client = CalcClient)]
impl Calc {
```

To:

```rust
#[contract]
impl Calc {
```

The auto-derived name will be `CalcClient` (from `Calc` + `Client`), which matches the existing re-export in `lib.rs`, so no other changes needed.

- [ ] **Step 2: Update the Cargo.toml description**

In `example-contracts/codegen_impl_noclient/Cargo.toml`, change:

```toml
description = "Codegen test: inherent impl, no client= attribute (no client generated)"
```

To:

```toml
description = "Codegen test: inherent impl, auto-derived client name"
```

- [ ] **Step 3: Build the example**

Run: `cargo check -p codegen_impl_noclient --features dev`
Expected: compiles with no errors. `CalcClient` is auto-derived from `Calc`.

- [ ] **Step 4: Commit**

```bash
git add example-contracts/codegen_impl_noclient/src/implementation.rs example-contracts/codegen_impl_noclient/Cargo.toml
git commit -m "codegen_impl_noclient: remove explicit client=, verify auto-derivation"
```

---

### Task 4: Full workspace build and verify

- [ ] **Step 1: Check the full workspace**

Run: `cargo check --workspace --features dev`
Expected: all crates compile

- [ ] **Step 2: Run tests**

Run: `cargo test --workspace --features dev`
Expected: all tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --features dev`
Expected: no new warnings
