# Replace hand-written spec model with XDR-generated `groundhog-spec`

## Goal

Replace the hand-written model types in `groundhog-contract-spec/src/model.rs` with
re-exports from the XDR-generated `groundhog-spec` registry package (v0.7.8). This
makes the XDR `.x` files the single source of truth for the wire format, so encoded
spec data can be read by any tool that understands the `.x` definitions.

The view types, traits (`ContractTypeSpec`, `ContractTypeName`, `ContractTypeRefSpec`),
`ContractTypeInfo`, const-eval helpers, and `groundhog_xdr_types.rs` impls all stay
local in `groundhog-contract-spec` -- they are SDK-specific and not part of the wire
format.

## Approach

Thin wrapper: `groundhog-contract-spec` depends on `groundhog-spec` and re-exports its
types in place of `model.rs`. All consumers update match arms / constructors to use the
generated variant names.

## Type shape changes

The generated types differ from the hand-written ones:

| Hand-written | Generated |
|---|---|
| `ContractType::Option(Box<CT>)` | `ContractType::Option { option_inner: Option<Box<CT>> }` |
| `ContractType::Vector(CVT)` | `ContractType::Vector { vector: CVT }` |
| `ContractType::Tuple(Vec<CT>)` | `ContractType::Tuple { tuple: VecM<CT> }` |
| `ContractType::Named(String)` | `ContractType::Named { named: StringM }` |
| `ContractType::Array(CAT)` | `ContractType::Array { array: CAT }` |
| `ContractTypeDef::Struct(d)` | `ContractTypeDef::TypeDefStruct { struct_def: d }` |
| `ContractTypeDef::Enum(d)` | `ContractTypeDef::TypeDefEnum { enum_def: d }` |
| `ContractTypeDef::Union(d)` | `ContractTypeDef::TypeDefUnion { union_def: d }` |
| `ContractUnionCaseType::Void` | `ContractUnionCaseType::UnionCaseVoid` |
| `ContractUnionCaseType::Tuple(v)` | `ContractUnionCaseType::UnionCaseTuple { tuple: v }` |
| `ContractUnionCaseType::Struct(v)` | `ContractUnionCaseType::UnionCaseStruct { struct_fields: v }` |
| `SpecFragment::Interface(f)` | `SpecFragment::SpecInterface { interface_fragment: f }` |
| `SpecFragment::Method(f)` | `SpecFragment::SpecMethod { method_fragment: f }` |
| `SpecFragment::TypeDef(d)` | `SpecFragment::SpecTypeDef { type_def: d }` |
| `String` fields | `ContractSpecString` (= `StringM`) |
| `Vec<T>` fields | `VecM<T>` |
| `Box<ContractType>` (in array/vector element) | `Option<Box<ContractType>>` |
| `ContractSpecInterface.version` | `ContractSpecInterface.vers` |

## Changes by file

### 1. Root `Cargo.toml`

Add workspace dependency:
```toml
groundhog-spec = { registry = "thruput", version = "0.7.8", default-features = false }
```

Update `alloc` feature to include `groundhog-spec/std` if needed (the generated crate
uses `std` as its allocation feature, not `alloc`).

### 2. `groundhog-contract-spec/Cargo.toml`

- Add `groundhog-spec = { workspace = true }` dependency
- May drop direct `xdrs` dep if all XDR types come transitively through `groundhog-spec`
  (verify during implementation -- the view types still derive `ConstWriteXDR`/`WriteXDR`
  so `xdrs` may still be needed directly)

### 3. `groundhog-contract-spec/src/model.rs`

Delete entirely.

### 4. `groundhog-contract-spec/src/lib.rs`

- Remove `pub mod model` / `pub use model::*`
- Add re-exports from `groundhog-spec` (ungated -- no `alloc` required):
  ```rust
  pub use groundhog_spec::{
      ContractArrayType, ContractVectorType, ContractType, ContractTypeKind,
      ContractField, ContractSpecMethod, ContractSpecInterface,
      ContractStructDef, ContractEnumVariant, ContractEnumDef,
      ContractUnionCaseType, ContractUnionCaseKind, ContractUnionCase, ContractUnionDef,
      ContractTypeDef, ContractTypeDefKind,
      ContractSpec, ContractSpecString,
      SpecInterfaceFragment, SpecMethodFragment, SpecFragment, SpecFragmentKind,
  };
  ```
- Keep `alloc` feature gating on:
  - `ContractTypeRefSpec` impls for `String`, `Vec<T>`, `VecM<T>`
  - All `From<...View> -> ...` conversions (they allocate)
- Keep all traits, `ContractTypeInfo`, const-eval helpers, `groundhog_xdr_types.rs` as-is

### 5. `groundhog-contract-spec/src/view.rs`

Update all `From<...View> -> Model` impls for new variant shapes:

- Enum constructors use struct variant syntax
- String conversions: `.to_owned()` -> `.into()` (to produce `StringM`)
- Vec conversions: need to produce `VecM` (likely `.collect::<Vec<_>>().into()` or
  build a `VecM` directly)
- `Box<CT>` -> `Option<Box<CT>>`: wrap in `Some(...)`
- `ContractSpecInterface` constructor: `.version` field -> `.vers`
- All `From` match destructures update to struct variant patterns

### 6. `groundhog-spec-tool/src/lib.rs`

- Update all match arms for `SpecFragment`, `ContractTypeDef`, `ContractType`,
  `ContractUnionCaseType` to use generated variant names
- Update constructors (e.g. `ContractSpecInterface { name, version, ... }` ->
  `{ name: x.into(), vers: y.into(), ... }`)
- String field access may need `.as_str()` where `&str` is expected from `StringM`
- Re-exports at top of file reference same types (no change needed since they come
  through `groundhog-contract-spec`)

### 7. `groundhog-spec-tool/src/raw.rs`

- Update `SpecFragment` match arms to generated variant names

### 8. No changes needed

- Macros (`macros/src/`) -- only reference view types, which are unchanged
- Example contracts -- only reference view types
- Standard interfaces -- only reference view types via macros
- Root SDK `src/lib.rs` -- re-exports unchanged crate name
- `groundhog_xdr_types.rs` -- only references view types and traits

## Testing

- `cargo check` across workspace
- `cargo test` in `groundhog-spec-tool`
- `cargo test` in root SDK
- Build example contracts to verify macro-generated code still works
