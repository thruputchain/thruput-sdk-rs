# Remove Transitive TYPE_DEFS from ContractTypeSpec

**Date:** 2026-04-15
**Status:** Approved

## Problem

`ContractTypeSpec::TYPE_DEFS` builds a transitive closure of all nested type definitions at const-eval time. This has three problems:

1. **Recursive types are impossible.** A struct like `Node { child: Option<Box<Node>> }` causes infinite const recursion because `Node::TYPE_DEFS` evaluates `type_defs::<Option<Box<Node>>>()` which needs `type_defs::<Node>()` which is `Node::TYPE_DEFS` again.

2. **Quadratic WASM bloat.** For a chain A embeds B embeds C: C emits `[C]`, B emits `[B, C]`, A emits `[A, B, C]`. Each `#[contract_type]` writes its full transitive closure. The spec tool deduplicates, but the raw section is O(n^2) in chain depth.

3. **Redundancy with the spec tool.** `resolve_reachable_type_defs` already walks struct fields and union cases to discover all transitively-referenced types. The compile-time closure duplicates this work.

## Solution

Each `#[contract_type]` emits exactly one `SpecFragmentView::TypeDef(Self::TYPE_DEF)` fragment -- just its own definition. The spec tool discovers transitive dependencies at canonicalization time (which it already does). The transitive const-eval machinery is deleted.

Additionally, add a blanket `ContractTypeRefSpec` impl for `Box<T>` so that recursive types like `Option<Box<Node>>` can appear in contract type fields and method signatures.

## Changes by Crate

### `groundhog-contract-spec`

**Delete:**
- `ContractTypeDefsView` enum (the `Empty`/`One`/`Many` tree)
- `ContractTypeSpec::TYPE_DEFS` associated const
- `ContractTypeInfo::type_defs` field
- `type_defs::<T>()` helper function
- `const_txs_typedef_fragment_closure_len()` function
- `const_txs_typedef_fragment_closure_bytes()` function  
- `const_write_txs_typedef_fragment_closure()` function
- `type_defs` field from every `ContractTypeRefSpec` impl (primitives, `Option<T>`, `Vec<T>`, `VecM<T, N>`, `[T; N]`, `&[u8]`, `&str`, `String`, all tuple impls, and the blanket `T: ContractTypeSpec` impl)

**Add:**
- `impl<T: ContractTypeRefSpec> ContractTypeRefSpec for Box<T>` -- delegates to `T::INFO` (transparent wrapper, same as the `&[u8]` → `[u8]` pattern)

**Simplify:**
- `ContractTypeInfo` becomes just `{ ty: ContractTypeView<'a> }` (single field)

### `macros` (`derive_contract_type_spec.rs`)

**Delete:**
- `struct_type_defs_tokens()` function
- `enum_type_defs_tokens()` function
- `enum_variant_type_defs_tokens()` function
- `fields_type_defs_tokens()` function

**Simplify:**
- `expand()` no longer generates a `TYPE_DEFS` const in the `ContractTypeSpec` impl
- `emit_raw_typedef_fragment()` writes `SpecFragmentView::TypeDef(Self::TYPE_DEF)` directly using `emit_raw_fragment_tokens_with_cfg`, instead of going through the `const_txs_typedef_fragment_closure_*` functions

### `gen` (`spec_reg.rs`)

**Delete:**
- `typedef_fragment_tokens_for_method()` function
- `typedef_fragment_tokens_for_type()` function
- `typedef_fragment_tokens_for_type_path()` function
- `emit_raw_typedef_fragment_stream_tokens()` function
- `emit_raw_typedef_fragment_stream_tokens_with_cfg()` function

**Simplify:**
- `generate_impl_raw_spec_fragments()` -- remove all typedef fragment collection and emission. The `typedef_fragments` vec, the `trait_throw_typedef_fragments` block, and the `typedef_fragment_tokens_for_method` loop are all removed. Only interface + method fragments are emitted.
- `generate_free_function_raw_spec_fragments()` -- same simplification.

### `gen` (`contract.rs`)

**Delete:**
- `contract_type_typedef_tokens()` function
- `throws_typedef_tokens()` function
- The `type_defs` field from `LoweredContractType` struct (leaving only `view`, so the struct can be replaced with a bare `TokenStream` or renamed to just return the view tokens directly)
- The `type_defs` assignment in `lower_via_type_ref_trait()`

### `macros` (`contract.rs` -- trait expansion)

**Delete:**
- The `__GROUNDHOG_IFACE_THROW_TYPE_DEFS` hidden const from trait expansion (line 76-79). This const stored the transitive typedef closure for exception types referenced by the trait's `throws` clause. Exception types are `#[contract_type]` themselves, so their fragments are already emitted independently.
- The `throws_typedef_tokens` variable and its usage that feeds into the above const.

### `groundhog-spec-tool`

No changes. `resolve_reachable_type_defs` already does the full graph walk through struct fields and union cases.

### New test: `example-contracts/recursive_type`

A new example contract proving the recursive case works:

```rust
#[contract_type]
pub struct Tree {
    pub value: u32,
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
}

#[contract(version = "0.1.0")]
impl RecursiveTypeContract {
    pub fn depth(tree: Tree) -> u32 { /* ... */ }
}
```

This contract must compile to WASM and produce a valid spec with the `Tree` type definition.

### Test updates

- **`groundhog-contract-spec/tests/roundtrip.rs`** -- Delete all tests that assert on `type_defs()` output or `ContractTypeDefsView` values. Update tests that reference `TYPE_DEFS` or `ContractTypeInfo::type_defs`. The `Payload` manual `ContractTypeSpec` impl in this file needs its `TYPE_DEFS` const removed.
- **`gen/src/spec_reg.rs` tests** -- Remove assertions about typedef fragment emission from method/impl fragment tests. Tests that check for `type_defs` or `maybe_type_def` in generated tokens should be deleted or updated.
- **`tests/contract_spec_reexport.rs`** -- Uses `ContractSpec::type_defs` (the final spec struct field), not `ContractTypeDefsView`. No changes needed.
- **`groundhog-spec-tool/tests/canonicalize.rs`** -- No changes needed. These tests construct `SpecFragment` values directly and test the spec tool's canonicalization, which is unchanged.

## What Does NOT Change

- The `ContractTypeSpec::TYPE_DEF` associated const -- still exists, still defines the type's shape
- The `ContractTypeName::NAME` trait -- unchanged
- The `ContractTypeRefSpec::INFO` trait -- still exists, just loses the `type_defs` field
- `type_ref::<T>()` helper -- still exists
- `SpecFragmentView` / `SpecFragment` enum variants -- unchanged
- The spec tool's canonicalization and reachability algorithm -- unchanged
- The `#[contract_type]` macro's attribute interface -- unchanged
- The final spec output format -- unchanged

## Why This Is Safe

The spec tool's `resolve_reachable_type_defs` starts from method signatures and exception types, then walks struct fields and union cases to discover all transitively-referenced named types, looking each up in the typedef map. This map is populated from ALL `SpecTypeDef` fragments in the WASM section. Since every `#[contract_type]` emits its own fragment (with `cfg(target_family = "wasm")`), every type that's compiled into the WASM binary has its definition present. The reachability walk finds them all.

The method-level typedef emission was a belt-and-suspenders guarantee that all types referenced by a method were present. With each `#[contract_type]` self-registering, this is redundant.
