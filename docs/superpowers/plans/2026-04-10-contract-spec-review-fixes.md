# Contract Spec Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the two approved contract-spec review issues: accept `i32::MIN` in `#[contract_error]` discriminants, and defer typedef conflict rejection until after reachable-type resolution.

**Architecture:** Keep both fixes surgical. The macro change stays local to `macros/src/contract_error.rs`, and the spec-tool change stays local to `groundhog-spec-tool/src/lib.rs` plus its existing unit tests. No alloc-diagnostic, symbol-identity, or entrypoint-version behavior changes are in scope.

**Tech Stack:** Rust, `syn`, proc macros, `cargo test`, existing `groundhog-spec-tool` canonicalization tests

---

## File Map

- Modify: `macros/src/contract_error.rs`
  - Own the `#[contract_error]` discriminant parser change and its focused unit test.
- Modify: `groundhog-spec-tool/src/lib.rs`
  - Move typedef conflict rejection from eager raw-scan time to reachable-resolution time.
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`
  - Flip the unreachable-conflict test and keep the reachable-conflict behavior pinned.

### Task 1: Make `#[contract_error]` Accept `i32::MIN`

**Files:**
- Modify: `macros/src/contract_error.rs`
- Test: `macros/src/contract_error.rs`

- [ ] **Step 1: Add the focused boundary test**

Add a unit test at the bottom of `macros/src/contract_error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::parse_i32_expr;

    #[test]
    fn parse_i32_expr_accepts_i32_min_literal() {
        let expr: syn::Expr = syn::parse_quote!(-2147483648);

        assert_eq!(parse_i32_expr(&expr).unwrap(), i32::MIN);
    }
}
```

- [ ] **Step 2: Run the targeted macro test and confirm it fails first**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros parse_i32_expr_accepts_i32_min_literal --offline
```

Expected: FAIL with an integer-overflow / “number too large to fit in target type” style error coming from `parse_i32_expr`.

- [ ] **Step 3: Fix negative discriminant parsing with the smallest change**

Update the unary-negation branch in `parse_i32_expr` inside `macros/src/contract_error.rs` so it parses the literal body as `i64`, negates in `i64`, and then converts to `i32`:

```rust
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => match expr.as_ref() {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(value),
                ..
            }) => {
                let value = value.base10_parse::<i64>()?;
                let negated = value.checked_neg().ok_or_else(|| {
                    syn::Error::new_spanned(
                        expr,
                        "#[contract_error] discriminants must fit in i32",
                    )
                })?;
                i32::try_from(negated).map_err(|_| {
                    syn::Error::new_spanned(
                        expr,
                        "#[contract_error] discriminants must fit in i32",
                    )
                })
            }
            _ => Err(syn::Error::new_spanned(
                expr,
                "#[contract_error] discriminants must be integer literals for contract specs",
            )),
        },
```

- [ ] **Step 4: Re-run the targeted macro test**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros parse_i32_expr_accepts_i32_min_literal --offline
```

Expected: PASS

- [ ] **Step 5: Run the full macro crate**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
```

Expected: PASS

- [ ] **Step 6: Commit the macro fix**

```bash
git add macros/src/contract_error.rs
git commit -m "fix: accept i32 min contract_error discriminants"
```

### Task 2: Reject Conflicting Typedefs Only After Reachability Resolution

**Files:**
- Modify: `groundhog-spec-tool/src/lib.rs`
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`

- [ ] **Step 1: Change the unreachable-conflict test to the desired behavior**

Update the existing test in `groundhog-spec-tool/tests/canonicalize.rs` from rejecting unreachable conflicts to succeeding with no emitted typedefs:

```rust
#[test]
fn canonicalize_spec_ignores_unreachable_conflicting_type_defs() {
    let spec = canonicalize_spec(&[
        SpecFragment::Interface(SpecInterfaceFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![],
        }),
        SpecFragment::Method(SpecMethodFragment {
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            method: ContractSpecMethod {
                name: "ping".into(),
                args: vec![],
                return_type: ContractType::Void,
            },
        }),
        SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
            name: "NeverUsed".into(),
            fields: vec![],
        })),
        SpecFragment::TypeDef(ContractTypeDef::Union(ContractUnionDef {
            name: "NeverUsed".into(),
            cases: vec![],
        })),
    ])
    .unwrap();

    assert!(spec.type_defs.is_empty());
}
```

- [ ] **Step 2: Run the targeted spec-tool tests and confirm the new test fails first**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool canonicalize_spec_ignores_unreachable_conflicting_type_defs --offline
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool canonicalize_spec_rejects_conflicting_type_defs --offline
```

Expected:
- first command: FAIL with `ConflictingTypeDef("NeverUsed")`
- second command: PASS

- [ ] **Step 3: Remove eager duplicate rejection from raw collection**

Delete the eager duplicate-symbol rejection loop in `groundhog-spec-tool/src/lib.rs`:

```rust
    for (symbol, defs) in &type_defs {
        if defs.len() > 1 {
            return Err(SpecToolError::ConflictingTypeDef(symbol.clone()));
        }
    }
```

Keep raw collection behavior otherwise unchanged:

```rust
            SpecFragment::TypeDef(type_def) => {
                let symbol = type_def_symbol(type_def).to_owned();
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
```

- [ ] **Step 4: Enforce conflicts only when a symbol is reachable**

Inside the reachable typedef resolution path in `groundhog-spec-tool/src/lib.rs`, ensure the symbol-resolution logic is:

```rust
match defs.as_slice() {
    [] => return Err(SpecToolError::MissingTypeDef(symbol.to_string())),
    [only] => only.clone(),
    _ => return Err(SpecToolError::ConflictingTypeDef(symbol.to_string())),
}
```

Use the existing reachable traversal rather than adding a second normalization pass. The key requirement is:
- unreachable duplicate symbols are ignored
- reachable duplicate symbols still fail unless they deduped to one unique def during raw collection

- [ ] **Step 5: Re-run the targeted spec-tool tests**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool canonicalize_spec_ignores_unreachable_conflicting_type_defs --offline
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool canonicalize_spec_rejects_conflicting_type_defs --offline
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool canonicalize_spec_rejects_missing_type_defs --offline
```

Expected:
- all three commands PASS

- [ ] **Step 6: Run the full spec-tool crate**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool --offline
```

Expected: PASS

- [ ] **Step 7: Commit the spec-tool fix**

```bash
git add groundhog-spec-tool/src/lib.rs groundhog-spec-tool/tests/canonicalize.rs
git commit -m "fix: prune unreachable conflicting typedefs"
```

### Task 3: Final Verification

**Files:**
- Modify: none
- Test: `macros/src/contract_error.rs`
- Test: `groundhog-spec-tool/src/lib.rs`
- Test: `groundhog-spec-tool/tests/canonicalize.rs`

- [ ] **Step 1: Run the focused verification set**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool --offline
```

Expected: both commands PASS

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: no diff, or only formatting changes

- [ ] **Step 3: Re-run the verification set after formatting**

Run:

```bash
env -u RUSTC_WRAPPER cargo test -p groundhog-macros --offline
env -u RUSTC_WRAPPER cargo test -p groundhog-spec-tool --offline
```

Expected: both commands PASS again

- [ ] **Step 4: Commit any final formatting adjustments**

```bash
git add macros/src/contract_error.rs groundhog-spec-tool/src/lib.rs groundhog-spec-tool/tests/canonicalize.rs
git commit -m "chore: format contract spec review fixes"
```

## Self-Review

### Spec coverage

The plan covers both approved spec items:

- `#[contract_error]` `i32::MIN` parsing: Task 1
- reachability-scoped typedef conflict rejection: Task 2

No extra work is planned for alloc diagnostics, symbol identity, or entrypoint version behavior.

### Placeholder scan

No TODO/TBD placeholders remain. Each task includes concrete files, commands, expected failures, expected passes, and commit messages.

### Type consistency

The plan consistently uses the existing names:

- `parse_i32_expr`
- `SpecToolError::ConflictingTypeDef`
- `SpecToolError::MissingTypeDef`
- `canonicalize_spec`

No new API names are introduced.
