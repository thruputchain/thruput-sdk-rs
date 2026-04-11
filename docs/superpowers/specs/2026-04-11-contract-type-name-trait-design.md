# `ContractTypeName` Trait Design

## Problem

The spec name for custom types comes from `stringify!(ident)` baked into the `ContractTypeDefView` const at derive time. This couples the Rust identifier to the spec name, with no way to:

- Keep a stable spec name across Rust renames
- Customize the spec name for newtypes wrapping imported types

## Solution

### `ContractTypeName` trait

New trait in `groundhog-contract-spec/src/lib.rs`:

```rust
pub trait ContractTypeName {
    const NAME: &'static str;
}
```

Independent of `ContractTypeSpec` — no supertrait relationship. Tuples, `Option<T>`, `Vec<T>`, etc. implement `ContractTypeRefSpec` without implementing either `ContractTypeName` or `ContractTypeSpec`.

### `#[derive(ContractTypeSpec)]` changes

The derive generates only `impl ContractTypeSpec`. It no longer generates `impl ContractTypeName`. The TYPE_DEF const uses `<Self as ContractTypeName>::NAME` instead of `stringify!(#ident)` for the type definition's name field.

Standalone derive users must impl `ContractTypeName` themselves.

### `#[contracttype]` changes

The attribute macro generates `impl ContractTypeName` directly (not via derive):

- `#[contracttype]` → `impl ContractTypeName { const NAME = stringify!(ident); }`
- `#[contracttype(name = "Custom")]` → `impl ContractTypeName { const NAME = "Custom"; }`

Then emits the item with `#[derive(ContractTypeSpec)]` as before.

### `#[contract_error]` changes

Same treatment. The attribute macro generates `impl ContractTypeName`:

- `#[contract_error]` → default name from ident
- `#[contract_error(name = "Custom")]` → custom name

The existing `version` argument continues to work alongside `name`.

## Scope

- Modify: `groundhog-contract-spec/src/lib.rs` — add `ContractTypeName` trait
- Modify: `macros/src/derive_contract_type_spec.rs` — remove `ContractTypeName` generation, use `<Self as ContractTypeName>::NAME` in TYPE_DEF
- Modify: `macros/src/contract_type.rs` — parse `name` arg, generate `impl ContractTypeName`
- Modify: `macros/src/contract_error.rs` — parse `name` arg, generate `impl ContractTypeName`
- Modify: `src/lib.rs` — re-export `ContractTypeName`
- Modify: `example-contracts/spec_named_types/src/implementation.rs` — no changes needed (attribute macro handles it)
- Update: `tests/compile-fail/contract_error_missing_discriminant.stderr` — if error output changes
