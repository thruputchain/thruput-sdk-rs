# Auto-derive client name for bare impls

**Date:** 2026-03-28

## Problem

A `#[contract]` on a bare `impl Foo` currently requires an explicit `client = FooClient` attribute, otherwise it emits a compile error. This is unnecessary — the client name can be auto-derived from the struct name, exactly as traits already auto-derive `{TraitName}Client`.

## Design

### Rules

| Item | `client` attribute | Behavior |
|---|---|---|
| `trait Foo` | omitted | auto-derive `FooClient` |
| `trait Foo` | `client = Bar` | use `Bar` |
| `impl Foo for Bar` | omitted | no client generated (trait is authoritative) |
| `impl Foo for Bar` | `client = X` | **compile error** (already exists, unchanged) |
| `impl Bar` (bare) | omitted | auto-derive `BarClient` **(new)** |
| `impl Bar` (bare) | `client = X` | use `X` (already works, unchanged) |

### Changes

**1. `gen/src/contract.rs` — `impl ContractSrc for syn::ItemImpl` — `client_ident()`**

Replace the always-`None` return with:
- If `self.trait_` is `Some`: return `None` (trait is authoritative source — unchanged).
- If `self.trait_` is `None` (bare impl): extract the last path segment from `self.self_ty`. Return `Some(format_ident!("{}Client", seg.ident))`.
- If `self_ty` is not a `Type::Path`: return `None`. The existing guard in `gen/src/client.rs` (`.ok_or_else(...)`) will produce a compile error.

**2. `macros/src/contract.rs` — impl branch client logic**

Remove the `else if item.trait_.is_none()` arm that emits `compile_error!("client = ClientName is required ...")`.

Simplify to:
- If `config.client_ident().is_some()` and `item.trait_.is_some()` → compile error (double-generation, unchanged).
- If bare impl (`item.trait_.is_none()`) → always call `codegen::client::generate(item, config)`.
- If trait impl with no explicit client → do nothing (client comes from trait definition, unchanged).

**3. Integration tests**

Verify that `codegen_impl_noclient` (which uses `#[contract(client = CalcClient)]` on a bare impl) still works. Add or modify a test case that omits `client` on a bare impl to confirm `{Struct}Client` is auto-derived.

### Not changing

- `ItemTrait::client_ident()` — already defaults correctly.
- `ConfiguredContract::client_ident()` fallback chain — already handles config-override → contract-default.
- `gen/src/client.rs` `.ok_or_else(...)` guard — stays as safety net for non-path `self_ty`.
- The compile error for `client = X` on `impl Trait for Struct` — stays.
- The compile error for `throws` on `impl Trait for Struct` — stays.
