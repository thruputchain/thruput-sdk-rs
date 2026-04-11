# `#[contracttype]` Macro Design

## Problem

Custom types used in contract method signatures (structs, plain enums, data-carrying enums) require three pieces of boilerplate:

1. `#[derive(ReadXDR, WriteXDR)]` with `#[xdrs(crate = "::sdk::__xdrs")]`
2. A manual `impl ContractTypeSpec` building `ContractTypeDefView` by hand
3. A manual `raw_typedefs` module emitting XDR fragments into the `groundhogspecrawv0` wasm section

Without these, the `#[contract]` macro generates `type_ref::<T>()` calls that fail because `T` doesn't satisfy `ContractTypeRefSpec` (via the blanket `T: ContractTypeSpec` → `ContractTypeRefSpec` impl).

`#[contract_error]` already automates all of this for error types (plus adding `Exn`), but there's no equivalent for plain data types.

## Solution

### `#[derive(ContractTypeSpec)]` — new derive macro

A derive macro that generates:

- `impl ContractTypeSpec` with `TYPE_DEF` and `TYPE_DEFS` constants
- Raw typedef wasm fragment emission (link_section into `groundhogspecrawv0`, guarded by `cfg(target_family = "wasm")`)

Supported item shapes:

| Shape | Spec representation |
|-------|-------------------|
| Struct (named fields) | `ContractTypeDefView::Struct` |
| Struct (unnamed fields) | `ContractTypeDefView::Struct` with `_0`, `_1`, ... field names |
| Enum (all unit variants, explicit discriminants) | `ContractTypeDefView::Enum` |
| Enum (data-carrying variants, explicit discriminants) | `ContractTypeDefView::Union` |

For `TYPE_DEFS`, the derive walks fields/variant payloads and collects transitive type definitions via `::sdk::__contract_spec::type_defs::<FieldType>()`, combining them with `ContractTypeDefsView::Many`.

Discriminant handling: all enum variants must have explicit `i32` discriminants (same requirement as `#[contract_error]`).

### `#[contracttype]` — new attribute macro

A thin wrapper that emits the annotated item with injected attributes:

- `#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]`
- `#[xdrs(crate = "::sdk::__xdrs")]`
- `#[repr(i32)]` for enums

### `#[contract_error]` refactoring

`contract_error.rs` stops generating `impl ContractTypeSpec` and raw typedef fragments inline. Instead, it injects `#[derive(ContractTypeSpec)]` into the item's derive list. It retains only:

- `Exn` impl with selector computation
- `#[repr(i32)]` injection for enums
- The other standard derives (Debug, Clone, PartialEq, ReadXDR, WriteXDR)

The shared codegen helpers (`enum_type_def_tokens`, `struct_type_def_tokens`, `enum_type_defs_tokens`, `struct_type_defs_tokens`, `emit_raw_typedef_fragment`, discriminant parsing, field/variant iteration) move from `contract_error.rs` into the new derive implementation.

## Demonstration

A minimal contract using a custom struct without `ContractTypeSpec` shows the compile error:

```rust
pub struct Payload { pub count: u32 }

#[contract(version = "0.1.0")]
impl Demo {
    pub fn echo(payload: Payload) -> Payload { payload }
}
```

This fails because `type_ref::<Payload>()` requires `Payload: ContractTypeRefSpec`, which requires `Payload: ContractTypeSpec`.

After the macro:

```rust
#[contracttype]
pub struct Payload { pub count: u32 }
```

generates all necessary impls and wasm fragments.

## Migration

The `spec_named_types` example contract replaces hand-written `ContractTypeSpec` impls and the manual `raw_typedefs` module with `#[contracttype]` on each type (`Payload`, `Choice`, `Envelope`).

## Scope

- New file: `macros/src/derive_contract_type_spec.rs` — derive macro implementation
- New file: `macros/src/contract_type.rs` — attribute macro (thin wrapper)
- Modified: `macros/src/contract_error.rs` — remove inline ContractTypeSpec codegen, inject derive instead
- Modified: `macros/src/lib.rs` — register new macros
- Modified: `src/lib.rs` — re-export `ContractTypeSpec` derive macro
- Modified: `example-contracts/spec_named_types/src/implementation.rs` — migrate to `#[contracttype]`
