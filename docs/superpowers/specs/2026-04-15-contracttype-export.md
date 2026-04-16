# `#[contracttype(export)]` — Force-Export Types in Spec

**Date:** 2026-04-15
**Status:** Approved

## Problem

The spec tool tree-shakes types, only including those reachable from method signatures and throws types. Types like `ChannelState` in the payment_channel contract are part of the contract's public interface (e.g., for off-chain signing) but aren't referenced in any method signature, so they get dropped from the spec.

## Solution

Add an `export` flag to `#[contracttype]`. When set, the type is always included in the spec output regardless of whether it's reachable from a method signature.

```rust
#[contracttype(export)]
pub struct ChannelState {
    pub channel_id: u64,
    pub balance_a: i128,
    pub balance_b: i128,
    pub nonce: u64,
}
```

The current default behavior (tree-shaken, only included if reachable) is unchanged. `#[contracttype]` without `export` works exactly as before.

## Mechanism

A new `SpecFragmentView` variant `ExportedTypeDef` (discriminant `= 3`) distinguishes force-exported types from regular typedefs in the raw spec section. The spec tool treats `ExportedTypeDef` fragments as both typedef definitions AND reachability roots.

## Changes by Crate

### `groundhog-spec` (external XDR crate)

Add `ExportedTypeDef` variant (discriminant 3) to the owned `SpecFragment` enum. Bump crate version.

### `groundhog-contract-spec/src/view.rs`

Add `ExportedTypeDef(ContractTypeDefView<'a>) = 3` to `SpecFragmentView`.

Add `From<SpecFragmentView>` conversion for the new variant (maps to `SpecFragment::ExportedTypeDef`).

### `macros/src/contract_type.rs`

Add `export: Option<bool>` to `ContractTypeArgs` (darling `FromMeta`). Pass `export` through to the derive via a `#[contract_type_spec(export)]` helper attribute on the item before derive expansion.

### `macros/src/derive_contract_type_spec.rs`

Check for the `#[contract_type_spec(export)]` helper attribute. When present, emit `SpecFragmentView::ExportedTypeDef(TYPE_DEF)` instead of `SpecFragmentView::TypeDef(TYPE_DEF)` in `emit_raw_typedef_fragment`.

### `groundhog-spec-tool/src/lib.rs`

In `canonicalize_spec`:
- Handle `SpecFragment::ExportedTypeDef` alongside `SpecFragment::SpecTypeDef`: add to the `type_defs` map.
- Track which type names came from `ExportedTypeDef` fragments.

In `resolve_reachable_type_defs`:
- Accept the set of exported type names as an additional parameter.
- Seed exported types as roots alongside method signatures and throws types.
- Exported types and their transitive dependencies are included in the output.

### `integration_test/contracts/payment_channel/src/interface.rs`

Add `export` to `ChannelState`:

```rust
#[contracttype(export)]
pub struct ChannelState { ... }
```

## What Does NOT Change

- Default `#[contracttype]` behavior (tree-shaken)
- The final spec output format (exported types appear identically to reachable types)
- The reachability algorithm (same walk, just additional roots)
- `#[contract_exception]` (no export flag needed — exceptions are always reachable via throws)
