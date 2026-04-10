# Contract Error Explicit Discriminants Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove proc-macro integer parsing from `#[contract_error]` enum spec generation by requiring explicit discriminants and splicing their expressions directly into generated Rust.

**Architecture:** Keep the change local to `macros/src/contract_error.rs` plus compile-fail coverage. Replace `enum_discriminants()` / `parse_i32_expr()` with an explicit-discriminant token path, make missing discriminants a macro error, and verify both the positive `-2147483648` case and the negative implicit-discriminant case.

**Tech Stack:** Rust, `syn`, proc macros, `trybuild`, existing compile-fail harness

---

## File Map

- Modify: `macros/src/contract_error.rs`
  - Delete proc-macro discriminant parsing, require explicit discriminants, and emit discriminant expressions directly into generated spec tokens.
- Modify: `tests/compile_fail.rs`
  - Only if needed for an exact targeted command during development; otherwise leave unchanged.
- Create: `tests/compile-fail/contract_error_missing_discriminant.rs`
  - Negative fixture for the new explicit-discriminant rule.
- Create: `tests/compile-fail/contract_error_missing_discriminant.stderr`
  - Expected compiler stderr for that fixture.

### Task 1: Replace Proc-Macro Discriminant Parsing With Direct Expression Emission

**Files:**
- Modify: `macros/src/contract_error.rs`

- [ ] **Step 1: Add a red unit test for explicit negative discriminants**

At the bottom of `macros/src/contract_error.rs`, replace the current parser-focused test with a codegen-focused test that proves a negative explicit discriminant is accepted and preserved in emitted tokens:

```rust
#[cfg(test)]
mod tests {
    use super::enum_type_def_tokens;

    #[test]
    fn enum_type_def_tokens_accepts_explicit_i32_min_discriminant() {
        let item: syn::ItemEnum = syn::parse_quote! {
            enum Error {
                No = -2147483648,
            }
        };

        let tokens = enum_type_def_tokens(&item).unwrap().to_string();

        assert!(tokens.contains("discriminant : - 2147483648"));
    }
}
```

If token spacing renders differently, adjust the assertion to match the actual `quote!` output, but keep the intent: the test should prove the emitted spec tokens carry the user expression instead of a proc-macro-parsed integer.

- [ ] **Step 2: Run the targeted unit test and confirm it fails first**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros enum_type_def_tokens_accepts_explicit_i32_min_discriminant --offline
```

Expected: FAIL on the current parser path, either because the old test is gone or because the old code cannot satisfy the new token-preservation assertion.

- [ ] **Step 3: Replace `enum_discriminants()` with an explicit-discriminant token helper**

In `macros/src/contract_error.rs`, delete:

- `enum_discriminants()`
- `parse_i32_expr()`

Replace them with a helper that returns discriminant expressions as token streams and errors on missing discriminants:

```rust
fn enum_discriminant_tokens(
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
) -> syn::Result<Vec<TokenStream>> {
    variants
        .iter()
        .map(|variant| {
            let Some((_, expr)) = &variant.discriminant else {
                return Err(syn::Error::new_spanned(
                    variant,
                    "#[contract_error] enum variants must use explicit discriminants for contract specs",
                ));
            };
            Ok(quote! { #expr })
        })
        .collect()
}
```

- [ ] **Step 4: Rewire enum spec generation to use discriminant token streams**

Update `enum_type_def_tokens()` in `macros/src/contract_error.rs`:

- replace `let discriminants = enum_discriminants(&item.variants)?;`
  with:

```rust
    let discriminants = enum_discriminant_tokens(&item.variants)?;
```

- keep the existing plain-enum vs union split
- keep the current unsupported multi-payload-field checks
- keep using:

```rust
value: #discriminant
```

and:

```rust
discriminant: #discriminant
```

so the original Rust expressions flow into generated code unchanged

- [ ] **Step 5: Remove old parser imports and dead tests**

Still in `macros/src/contract_error.rs`:

- remove `use std::convert::TryFrom;`
- remove the old `parse_i32_expr` test
- ensure the new unit test is the only local test block needed for this change

- [ ] **Step 6: Run the targeted unit test**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros enum_type_def_tokens_accepts_explicit_i32_min_discriminant --offline
```

Expected: PASS

- [ ] **Step 7: Run the full macro crate**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
```

Expected: PASS

- [ ] **Step 8: Commit Task 1**

```bash
git add macros/src/contract_error.rs
git commit -m "refactor: emit explicit contract_error discriminants"
```

### Task 2: Add Compile-Fail Coverage For Missing Discriminants

**Files:**
- Create: `tests/compile-fail/contract_error_missing_discriminant.rs`
- Create: `tests/compile-fail/contract_error_missing_discriminant.stderr`

- [ ] **Step 1: Add the failing compile fixture**

Create `tests/compile-fail/contract_error_missing_discriminant.rs`:

```rust
use sdk::contract_error;

#[contract_error]
enum Error {
    One = 1,
    Two,
}

fn main() {}
```

- [ ] **Step 2: Run the compile-fail harness to capture the new stderr**

Run:

```bash
env -u RUSTC_WRAPPER cargo test --test compile_fail --offline
```

Expected: FAIL with a new trybuild output telling you to bless or update stderr for `contract_error_missing_discriminant.rs`.

- [ ] **Step 3: Save the expected stderr**

Create `tests/compile-fail/contract_error_missing_discriminant.stderr` with the exact emitted diagnostic from the failing run, making sure it includes:

- the span on the missing-discriminant variant
- the message:

```text
#[contract_error] enum variants must use explicit discriminants for contract specs
```

- [ ] **Step 4: Re-run the compile-fail harness**

Run:

```bash
env -u RUSTC_WRAPPER cargo test --test compile_fail --offline
```

Expected: PASS

- [ ] **Step 5: Commit Task 2**

```bash
git add tests/compile-fail/contract_error_missing_discriminant.rs tests/compile-fail/contract_error_missing_discriminant.stderr
git commit -m "test: cover missing contract_error discriminants"
```

### Task 3: Final Verification

**Files:**
- Modify: none

- [ ] **Step 1: Run the focused verification set**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
env -u RUSTC_WRAPPER cargo test --test compile_fail --offline
```

Expected: both PASS

- [ ] **Step 2: Format the tree**

Run:

```bash
cargo fmt --all
```

Expected: no diff, or only formatting changes

- [ ] **Step 3: Re-run verification after formatting**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
env -u RUSTC_WRAPPER cargo test --test compile_fail --offline
```

Expected: both PASS again

- [ ] **Step 4: Commit any final formatting**

```bash
git add macros/src/contract_error.rs tests/compile-fail/contract_error_missing_discriminant.rs tests/compile-fail/contract_error_missing_discriminant.stderr
git commit -m "chore: format contract_error discriminant changes"
```

## Self-Review

### Spec coverage

The plan covers the approved redesign:

- no proc-macro integer parser
- explicit discriminants required for `#[contract_error]` enums
- discriminant expressions copied directly into generated Rust
- compile-fail coverage for the new missing-discriminant rule

### Placeholder scan

No TODO/TBD placeholders remain. Each task includes exact file paths, concrete code, commands, expected failures, expected passes, and commit messages.

### Type consistency

The plan consistently uses:

- `enum_type_def_tokens`
- `enum_discriminant_tokens`
- `#[contract_error] enum variants must use explicit discriminants for contract specs`

It intentionally removes `enum_discriminants()` and `parse_i32_expr()` rather than renaming them.
