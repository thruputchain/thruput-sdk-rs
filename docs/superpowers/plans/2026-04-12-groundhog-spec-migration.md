# Migrate to XDR-generated `groundhog-spec` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hand-written model types in `groundhog-contract-spec/src/model.rs` with re-exports from the XDR-generated `groundhog-spec` registry package, making the `.x` files the canonical wire format.

**Architecture:** `groundhog-contract-spec` becomes a thin wrapper that re-exports XDR-generated types from `groundhog-spec` and adds SDK-specific view types, traits, and const-eval helpers. The `groundhog-spec` dependency is optional, gated on `alloc` (since the generated types contain `Vec`-based containers). All consumers update match arms and constructors to use the generated enum variant names.

**Tech Stack:** Rust, `groundhog-spec` 0.7.8 (groundhog registry), `xdrs`/`xdr-common` for `VecM`/`StringM` types.

---

### Task 1: Update dependency configuration

**Files:**
- Modify: `Cargo.toml` (root workspace)
- Modify: `groundhog-contract-spec/Cargo.toml`

- [ ] **Step 1: Add `groundhog-spec` to workspace dependencies**

In root `Cargo.toml`, add under `[workspace.dependencies]`:

```toml
groundhog-spec = { registry = "groundhog", version = "0.7.8", default-features = false }
```

- [ ] **Step 2: Add `groundhog-spec` as optional dep and update features in `groundhog-contract-spec/Cargo.toml`**

```toml
[features]
default = ["alloc"]
alloc = ["xdrs/alloc", "dep:groundhog-spec"]

[dependencies]
groundhog-xdr = { workspace = true }
groundhog-spec = { workspace = true, optional = true }
xdrs = { workspace = true, features = ["heapless", "derive"] }
```

`groundhog-spec` is optional because the generated types contain `VecM`/`StringM` (which wrap `Vec`), requiring alloc. Without `alloc`, only the view types (pure `no_std`/`no_alloc`) are available — matching the existing gating of `model.rs`.

- [ ] **Step 3: Verify dependency resolves**

Run: `cargo check -p groundhog-contract-spec --features alloc 2>&1 | head -20`
Expected: downloads `groundhog-spec` and compiles (model.rs still exists at this point so there will be type conflicts — that's OK, just verify the dep resolves)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock groundhog-contract-spec/Cargo.toml
git commit -m "Add groundhog-spec dependency for XDR-generated spec types"
```

---

### Task 2: Replace model.rs with re-exports

**Files:**
- Delete: `groundhog-contract-spec/src/model.rs`
- Modify: `groundhog-contract-spec/src/lib.rs`

- [ ] **Step 1: Delete `model.rs`**

```bash
rm groundhog-contract-spec/src/model.rs
```

- [ ] **Step 2: Update `lib.rs` — replace model imports with re-exports from `groundhog-spec`**

Replace the current `model.rs`-related code:
```rust
#[cfg(feature = "alloc")]
pub mod model;
...
#[cfg(feature = "alloc")]
pub use model::*;
```

With re-exports from `groundhog-spec`:
```rust
#[cfg(feature = "alloc")]
pub use groundhog_spec::{
    ContractArrayType, ContractField, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractSpecString, ContractStructDef, ContractType,
    ContractTypeDefKind, ContractTypeKind, ContractUnionCase,
    ContractUnionCaseKind, ContractUnionCaseType, ContractUnionDef,
    ContractVectorType,
    ContractEnumDef, ContractEnumVariant, ContractTypeDef,
    SpecFragment, SpecFragmentKind, SpecInterfaceFragment, SpecMethodFragment,
};
```

Also update the `VecM` import — currently:
```rust
#[cfg(feature = "alloc")]
use xdrs::VecM;
```
This is no longer needed for `model.rs` but may still be needed for `ContractTypeRefSpec` impls. Keep it if used, remove if not.

- [ ] **Step 3: Verify `lib.rs` compiles (ignoring downstream — view.rs will break)**

Run: `cargo check -p groundhog-contract-spec --features alloc 2>&1 | head -40`
Expected: errors in `view.rs` about mismatched types (the `From` impls reference old variant names). This is expected — Task 3 fixes those.

- [ ] **Step 4: Commit**

```bash
git add groundhog-contract-spec/src/lib.rs
git rm groundhog-contract-spec/src/model.rs
git commit -m "Replace hand-written model.rs with re-exports from groundhog-spec"
```

---

### Task 3: Update view.rs From conversions

**Files:**
- Modify: `groundhog-contract-spec/src/view.rs`

The view types themselves (`ContractTypeView`, `ContractTypeDefView`, etc.) are unchanged. Only the `#[cfg(feature = "alloc")] From<...View> -> Model` impls need updating for the new generated type shapes.

Key changes across all `From` impls:
- Enum variants use struct syntax: `ContractType::Option { option_inner: ... }` not `ContractType::Option(...)`
- `Box<ContractType>` → `Option<Box<ContractType>>` (wrap in `Some(...)`)
- String conversions: `.to_owned()` → `ContractSpecString::try_from(s).unwrap()` (or `ContractSpecString(s.as_bytes().to_vec())`)
- Vec conversions: `.collect()` → collect into `Vec<T>` then `.try_into().unwrap()` for `VecM<T>`
- `ContractSpecInterface.version` → `.vers`
- `ContractTypeDef::Struct(d)` → `ContractTypeDef::TypeDefStruct { struct_def: d }`
- `ContractUnionCaseType::Void` → `ContractUnionCaseType::UnionCaseVoid`
- `SpecFragment::Interface(f)` → `SpecFragment::SpecInterface { interface_fragment: f }`

- [ ] **Step 1: Update imports at top of `view.rs`**

Replace:
```rust
#[cfg(feature = "alloc")]
use crate::model::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractStructDef,
    ContractType, ContractTypeDef, ContractUnionCase, ContractUnionDef,
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
#[cfg(feature = "alloc")]
use alloc::{borrow::ToOwned, boxed::Box};
```

With:
```rust
#[cfg(feature = "alloc")]
use crate::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractSpecString,
    ContractStructDef, ContractType, ContractTypeDef, ContractUnionCase,
    ContractUnionCaseType, ContractUnionDef,
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
#[cfg(feature = "alloc")]
use alloc::{boxed::Box, vec::Vec};
```

- [ ] **Step 2: Add helper for `&str` → `ContractSpecString` conversion**

Add at the top of the `#[cfg(feature = "alloc")]` section:
```rust
#[cfg(feature = "alloc")]
fn to_spec_string(s: &str) -> ContractSpecString {
    ContractSpecString::try_from(s).expect("spec string conversion")
}
```

- [ ] **Step 3: Update `From<ContractSpecView> for ContractSpec`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractSpecView<'_>> for ContractSpec {
    fn from(value: ContractSpecView<'_>) -> Self {
        Self {
            interfaces: value
                .interfaces
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            type_defs: value
                .type_defs
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}
```

- [ ] **Step 4: Update `From<ContractSpecInterfaceView> for ContractSpecInterface`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractSpecInterfaceView<'_>> for ContractSpecInterface {
    fn from(value: ContractSpecInterfaceView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            vers: to_spec_string(value.version),
            throws: value.throws.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
            methods: value.methods.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
        }
    }
}
```

- [ ] **Step 5: Update `From<ContractSpecMethodView> for ContractSpecMethod`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractSpecMethodView<'_>> for ContractSpecMethod {
    fn from(value: ContractSpecMethodView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            args: value.args.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
            return_type: value.return_type.into(),
        }
    }
}
```

- [ ] **Step 6: Update `From<ContractFieldView> for ContractField`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractFieldView<'_>> for ContractField {
    fn from(value: ContractFieldView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            ty: value.ty.into(),
        }
    }
}
```

- [ ] **Step 7: Update `From<ContractTypeView> for ContractType`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractTypeView<'_>> for ContractType {
    fn from(value: ContractTypeView<'_>) -> Self {
        match value {
            ContractTypeView::Void => ContractType::Void,
            ContractTypeView::Bool => ContractType::Bool,
            ContractTypeView::U32 => ContractType::U32,
            ContractTypeView::I32 => ContractType::I32,
            ContractTypeView::U64 => ContractType::U64,
            ContractTypeView::I64 => ContractType::I64,
            ContractTypeView::U128 => ContractType::U128,
            ContractTypeView::I128 => ContractType::I128,
            ContractTypeView::String => ContractType::String,
            ContractTypeView::Bytes => ContractType::Bytes,
            ContractTypeView::Address => ContractType::Address,
            ContractTypeView::Option(inner) => {
                ContractType::Option {
                    option_inner: Some(Box::new((*inner).into())),
                }
            }
            ContractTypeView::Vector(inner) => {
                ContractType::Vector { vector: inner.into() }
            }
            ContractTypeView::Tuple(items) => ContractType::Tuple {
                tuple: items.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
            },
            ContractTypeView::Named(name) => {
                ContractType::Named { named: to_spec_string(name) }
            }
            ContractTypeView::U8 => ContractType::U8,
            ContractTypeView::Array(array) => {
                ContractType::Array { array: array.into() }
            }
        }
    }
}
```

- [ ] **Step 8: Update `From<ContractArrayTypeView>` and `From<ContractVectorTypeView>`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractArrayTypeView<'_>> for crate::ContractArrayType {
    fn from(value: ContractArrayTypeView<'_>) -> Self {
        Self {
            element: Some(Box::new((*value.element).into())),
            len: value.len,
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractVectorTypeView<'_>> for crate::ContractVectorType {
    fn from(value: ContractVectorTypeView<'_>) -> Self {
        Self {
            element: Some(Box::new((*value.element).into())),
            max_len: value.max_len,
        }
    }
}
```

- [ ] **Step 9: Update `From<ContractTypeDefView> for ContractTypeDef`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractTypeDefView<'_>> for ContractTypeDef {
    fn from(value: ContractTypeDefView<'_>) -> Self {
        match value {
            ContractTypeDefView::Struct(def) => {
                ContractTypeDef::TypeDefStruct { struct_def: def.into() }
            }
            ContractTypeDefView::Enum(def) => {
                ContractTypeDef::TypeDefEnum { enum_def: def.into() }
            }
            ContractTypeDefView::Union(def) => {
                ContractTypeDef::TypeDefUnion { union_def: def.into() }
            }
        }
    }
}
```

- [ ] **Step 10: Update `From<ContractStructDefView>`, `From<ContractEnumDefView>`, `From<ContractEnumVariantView>`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractStructDefView<'_>> for ContractStructDef {
    fn from(value: ContractStructDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            fields: value.fields.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractEnumDefView<'_>> for ContractEnumDef {
    fn from(value: ContractEnumDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            variants: value.variants.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractEnumVariantView<'_>> for ContractEnumVariant {
    fn from(value: ContractEnumVariantView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            value: value.value,
        }
    }
}
```

- [ ] **Step 11: Update `From<ContractUnionDefView>`, `From<ContractUnionCaseView>`, `From<ContractUnionCaseTypeView>`**

```rust
#[cfg(feature = "alloc")]
impl From<ContractUnionDefView<'_>> for ContractUnionDef {
    fn from(value: ContractUnionDefView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            cases: value.cases.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractUnionCaseView<'_>> for ContractUnionCase {
    fn from(value: ContractUnionCaseView<'_>) -> Self {
        Self {
            name: to_spec_string(value.name),
            discriminant: value.discriminant,
            case_type: value.case_type.into(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<ContractUnionCaseTypeView<'_>> for ContractUnionCaseType {
    fn from(value: ContractUnionCaseTypeView<'_>) -> Self {
        match value {
            ContractUnionCaseTypeView::Void => ContractUnionCaseType::UnionCaseVoid,
            ContractUnionCaseTypeView::Tuple(types) => {
                ContractUnionCaseType::UnionCaseTuple {
                    tuple: types.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
                }
            }
            ContractUnionCaseTypeView::Struct(fields) => {
                ContractUnionCaseType::UnionCaseStruct {
                    struct_fields: fields.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
                }
            }
        }
    }
}
```

- [ ] **Step 12: Update `From<SpecFragmentView>`, `From<SpecInterfaceFragmentView>`, `From<SpecMethodFragmentView>`**

```rust
#[cfg(feature = "alloc")]
impl From<SpecFragmentView<'_>> for SpecFragment {
    fn from(value: SpecFragmentView<'_>) -> Self {
        match value {
            SpecFragmentView::Interface(fragment) => {
                SpecFragment::SpecInterface { interface_fragment: fragment.into() }
            }
            SpecFragmentView::Method(fragment) => {
                SpecFragment::SpecMethod { method_fragment: fragment.into() }
            }
            SpecFragmentView::TypeDef(def) => {
                SpecFragment::SpecTypeDef { type_def: def.into() }
            }
        }
    }
}

#[cfg(feature = "alloc")]
impl From<SpecInterfaceFragmentView<'_>> for SpecInterfaceFragment {
    fn from(value: SpecInterfaceFragmentView<'_>) -> Self {
        Self {
            interface_name: to_spec_string(value.interface_name),
            interface_version: to_spec_string(value.interface_version),
            throws: value.throws.iter().copied().map(Into::into).collect::<Vec<_>>().try_into().unwrap(),
        }
    }
}

#[cfg(feature = "alloc")]
impl From<SpecMethodFragmentView<'_>> for SpecMethodFragment {
    fn from(value: SpecMethodFragmentView<'_>) -> Self {
        Self {
            interface_name: to_spec_string(value.interface_name),
            interface_version: to_spec_string(value.interface_version),
            method: value.method.into(),
        }
    }
}
```

- [ ] **Step 13: Verify `groundhog-contract-spec` compiles**

Run: `cargo check -p groundhog-contract-spec --features alloc 2>&1 | tail -5`
Expected: compiles successfully (may have warnings)

- [ ] **Step 14: Commit**

```bash
git add groundhog-contract-spec/src/view.rs
git commit -m "Update view-to-model From impls for XDR-generated type shapes"
```

---

### Task 4: Update spec-tool `lib.rs`

**Files:**
- Modify: `groundhog-spec-tool/src/lib.rs`

The spec-tool heavily uses the model types for canonicalization and JSON rendering. Key changes:
- All enum match arms update to struct variant syntax
- `ContractSpecString` (= `StringM`, wraps `Vec<u8>`) replaces `String` in type fields
- `VecM<T>` replaces `Vec<T>` in type fields
- `ContractSpecInterface.version` → `.vers`
- `BTreeMap` keys change from `String`/`(String, String)` to `ContractSpecString`/`(ContractSpecString, ContractSpecString)`
- Error message formatting needs `str` conversion from `ContractSpecString`
- JSON rendering needs `str` conversion from `ContractSpecString`

- [ ] **Step 1: Update imports and add string helper**

Update the re-exports at the top:
```rust
pub use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpec,
    ContractSpecInterface, ContractSpecMethod, ContractSpecString,
    ContractStructDef, ContractType, ContractTypeDef, ContractUnionCase,
    ContractUnionCaseType, ContractUnionDef, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
```

Add a helper function for converting `ContractSpecString` to `&str`:
```rust
fn spec_str(s: &ContractSpecString) -> &str {
    std::str::from_utf8(s).expect("contract spec strings must be valid UTF-8")
}
```

Note: `StringM` derefs to `Vec<u8>`, and `Vec<u8>` derefs to `[u8]`, so `std::str::from_utf8(s)` works via `Deref` coercion.

- [ ] **Step 2: Update `InterfaceBucket` and `canonicalize_spec` — key types and fragment matching**

Change `BTreeMap` key types:
```rust
let mut interfaces = BTreeMap::<(ContractSpecString, ContractSpecString), InterfaceBucket>::new();
let mut type_defs = BTreeMap::<ContractSpecString, Vec<ContractTypeDef>>::new();
```

Update the fragment matching loop:
```rust
for fragment in raw {
    match fragment {
        SpecFragment::SpecInterface { interface_fragment: fragment } => {
            validate_interface_key(
                spec_str(&fragment.interface_name),
                spec_str(&fragment.interface_version),
            )?;

            let key = (
                fragment.interface_name.clone(),
                fragment.interface_version.clone(),
            );
            let throws = canonicalize_type_list(&fragment.throws);
            let bucket = interfaces.entry(key.clone()).or_default();

            match &bucket.throws {
                Some(existing) if *existing != throws => {
                    return Err(
                        SpecToolError::ConflictingInterfaceThrows(
                            interface_key_string(
                                spec_str(&key.0),
                                spec_str(&key.1),
                            ),
                        ),
                    );
                }
                Some(_) => {}
                None => bucket.throws = Some(throws),
            }
        }
        SpecFragment::SpecMethod { method_fragment: fragment } => {
            validate_interface_key(
                spec_str(&fragment.interface_name),
                spec_str(&fragment.interface_version),
            )?;
            validate_method(&fragment.method)?;

            let key = (
                fragment.interface_name.clone(),
                fragment.interface_version.clone(),
            );
            interfaces
                .entry(key)
                .or_default()
                .methods
                .push(fragment.method.clone());
        }
        SpecFragment::SpecTypeDef { type_def } => {
            let symbol = type_def_symbol(type_def).clone();
            let defs = type_defs.entry(symbol).or_default();
            if !defs.iter().any(|existing| existing == type_def) {
                defs.push(type_def.clone());
            }
        }
    }
}
```

- [ ] **Step 3: Update `canonicalize_spec` — interface assembly and output construction**

Update the conflict check and interface assembly:
```rust
for (symbol, defs) in &type_defs {
    if defs.len() > 1 {
        return Err(SpecToolError::ConflictingTypeDef(
            spec_str(symbol).to_owned(),
        ));
    }
}

let interfaces = interfaces
    .into_iter()
    .map(|((name, vers), mut bucket)| {
        bucket.methods.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                canonical_method_key(left)
                    .cmp(&canonical_method_key(right))
            })
        });
        let mut deduped_methods: Vec<ContractSpecMethod> =
            Vec::with_capacity(bucket.methods.len());
        for method in bucket.methods {
            if let Some(existing) = deduped_methods.last()
                && existing.name == method.name
            {
                if existing == &method {
                    continue;
                }
                return Err(SpecToolError::ConflictingMethod(format!(
                    "{}@{}::{}",
                    spec_str(&name),
                    spec_str(&vers),
                    spec_str(&method.name),
                )));
            }
            deduped_methods.push(method);
        }

        let throws = bucket.throws.ok_or_else(|| {
            SpecToolError::MissingInterfaceFragment(interface_key_string(
                spec_str(&name),
                spec_str(&vers),
            ))
        })?;

        Ok(ContractSpecInterface {
            name,
            vers,
            throws: throws.try_into().unwrap(),
            methods: deduped_methods.try_into().unwrap(),
        })
    })
    .collect::<Result<Vec<_>, _>>()?;

let type_defs = resolve_reachable_type_defs(&interfaces, &type_defs)?;
Ok(ContractSpec {
    interfaces: interfaces.try_into().unwrap(),
    type_defs: type_defs.try_into().unwrap(),
})
```

- [ ] **Step 4: Update `canonicalize_type_list`**

```rust
fn canonicalize_type_list(types: &[ContractType]) -> Vec<ContractType> {
    let mut types = types.to_vec();
    types.sort_by(|left, right| {
        canonical_type_key(left).cmp(&canonical_type_key(right))
    });
    types.dedup();
    types
}
```

Note: The input changes from `&[ContractType]` to `&VecM<ContractType>` which derefs to `&Vec<ContractType>` which derefs to `&[ContractType]` — so the function signature can stay the same via deref coercion. The function returns `Vec<ContractType>` which is used before converting to `VecM` at the construction site.

- [ ] **Step 5: Update `resolve_reachable_type_defs`**

Update the signature and type-def matching:
```rust
fn resolve_reachable_type_defs(
    interfaces: &[ContractSpecInterface],
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
) -> Result<Vec<ContractTypeDef>, SpecToolError> {
    let mut resolved = BTreeMap::<ContractSpecString, ContractTypeDef>::new();
    let mut pending = Vec::<ContractSpecString>::new();

    for interface in interfaces {
        for ty in interface.throws.iter() {
            collect_named_types(ty, defs, &mut resolved, &mut pending)?;
        }
        for method in interface.methods.iter() {
            for arg in method.args.iter() {
                collect_named_types(
                    &arg.ty,
                    defs,
                    &mut resolved,
                    &mut pending,
                )?;
            }
            collect_named_types(
                &method.return_type,
                defs,
                &mut resolved,
                &mut pending,
            )?;
        }
    }

    while let Some(symbol) = pending.pop() {
        match resolved.get(&symbol).cloned() {
            Some(ContractTypeDef::TypeDefStruct { struct_def: def }) => {
                for field in def.fields.iter() {
                    collect_named_types(
                        &field.ty,
                        defs,
                        &mut resolved,
                        &mut pending,
                    )?;
                }
            }
            Some(ContractTypeDef::TypeDefEnum { .. }) => {}
            Some(ContractTypeDef::TypeDefUnion { union_def: def }) => {
                for case in def.cases.iter() {
                    collect_union_case_named_types(
                        &case.case_type,
                        defs,
                        &mut resolved,
                        &mut pending,
                    )?;
                }
            }
            None => return Err(SpecToolError::MissingTypeDef(
                spec_str(&symbol).to_owned(),
            )),
        }
    }

    Ok(resolved.into_values().collect())
}
```

- [ ] **Step 6: Update `collect_named_types`**

```rust
fn collect_named_types(
    ty: &ContractType,
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    resolved: &mut BTreeMap<ContractSpecString, ContractTypeDef>,
    pending: &mut Vec<ContractSpecString>,
) -> Result<(), SpecToolError> {
    match ty {
        ContractType::Option { option_inner } => {
            if let Some(inner) = option_inner {
                collect_named_types(inner, defs, resolved, pending)?;
            }
        }
        ContractType::Vector { vector } => {
            if let Some(element) = &vector.element {
                collect_named_types(element, defs, resolved, pending)?;
            }
        }
        ContractType::Array { array } => {
            if let Some(element) = &array.element {
                collect_named_types(element, defs, resolved, pending)?;
            }
        }
        ContractType::Tuple { tuple } => {
            for item in tuple.iter() {
                collect_named_types(item, defs, resolved, pending)?;
            }
        }
        ContractType::Named { named } => {
            let Some(candidates) = defs.get(named) else {
                return Err(SpecToolError::MissingTypeDef(
                    spec_str(named).to_owned(),
                ));
            };
            if candidates.len() > 1 {
                return Err(SpecToolError::ConflictingTypeDef(
                    spec_str(named).to_owned(),
                ));
            }
            if !resolved.contains_key(named) {
                resolved.insert(named.clone(), candidates[0].clone());
                pending.push(named.clone());
            }
        }
        ContractType::Void
        | ContractType::Bool
        | ContractType::U8
        | ContractType::U32
        | ContractType::I32
        | ContractType::U64
        | ContractType::I64
        | ContractType::U128
        | ContractType::I128
        | ContractType::String
        | ContractType::Bytes
        | ContractType::Address => {}
        _ => {}
    }

    Ok(())
}
```

Note the `_ => {}` catch-all at the end — the generated enum is `#[non_exhaustive]`.

- [ ] **Step 7: Update `collect_union_case_named_types`**

```rust
fn collect_union_case_named_types(
    case_type: &ContractUnionCaseType,
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    resolved: &mut BTreeMap<ContractSpecString, ContractTypeDef>,
    pending: &mut Vec<ContractSpecString>,
) -> Result<(), SpecToolError> {
    match case_type {
        ContractUnionCaseType::UnionCaseVoid => {}
        ContractUnionCaseType::UnionCaseTuple { tuple } => {
            for ty in tuple.iter() {
                collect_named_types(ty, defs, resolved, pending)?;
            }
        }
        ContractUnionCaseType::UnionCaseStruct { struct_fields } => {
            for field in struct_fields.iter() {
                collect_named_types(&field.ty, defs, resolved, pending)?;
            }
        }
        _ => {}
    }
    Ok(())
}
```

- [ ] **Step 8: Update `canonical_method_key` and `canonical_type_key`**

```rust
fn canonical_method_key(method: &ContractSpecMethod) -> String {
    let mut key = spec_str(&method.name).to_owned();
    key.push('|');
    for arg in method.args.iter() {
        key.push_str(spec_str(&arg.name));
        key.push(':');
        key.push_str(&canonical_type_key(&arg.ty));
        key.push('|');
    }
    key.push_str("->");
    key.push_str(&canonical_type_key(&method.return_type));
    key
}

fn canonical_type_key(ty: &ContractType) -> String {
    match ty {
        ContractType::Void => "void".to_string(),
        ContractType::Bool => "bool".to_string(),
        ContractType::U8 => "u8".to_string(),
        ContractType::U32 => "u32".to_string(),
        ContractType::I32 => "i32".to_string(),
        ContractType::U64 => "u64".to_string(),
        ContractType::I64 => "i64".to_string(),
        ContractType::U128 => "u128".to_string(),
        ContractType::I128 => "i128".to_string(),
        ContractType::String => "string".to_string(),
        ContractType::Bytes => "bytes".to_string(),
        ContractType::Address => "address".to_string(),
        ContractType::Option { option_inner } => {
            let inner_key = option_inner
                .as_ref()
                .map(|inner| canonical_type_key(inner))
                .unwrap_or_default();
            format!("option({inner_key})")
        }
        ContractType::Vector { vector } => {
            let element_key = vector.element
                .as_ref()
                .map(|e| canonical_type_key(e))
                .unwrap_or_default();
            format!("vector({element_key}, {})", vector.max_len)
        }
        ContractType::Tuple { tuple } => {
            let mut key = String::from("tuple(");
            for (index, item) in tuple.iter().enumerate() {
                if index > 0 {
                    key.push(',');
                }
                key.push_str(&canonical_type_key(item));
            }
            key.push(')');
            key
        }
        ContractType::Array { array } => {
            let element_key = array.element
                .as_ref()
                .map(|e| canonical_type_key(e))
                .unwrap_or_default();
            format!("array({element_key}, {})", array.len)
        }
        ContractType::Named { named } => {
            format!("named({})", spec_str(named))
        }
        _ => "unknown".to_string(),
    }
}
```

- [ ] **Step 9: Update `type_def_symbol`**

```rust
fn type_def_symbol(type_def: &ContractTypeDef) -> &ContractSpecString {
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def } => &struct_def.name,
        ContractTypeDef::TypeDefEnum { enum_def } => &enum_def.name,
        ContractTypeDef::TypeDefUnion { union_def } => &union_def.name,
        _ => panic!("unknown ContractTypeDef variant"),
    }
}
```

- [ ] **Step 10: Update `validate_method`**

```rust
fn validate_method(method: &ContractSpecMethod) -> Result<(), SpecToolError> {
    if method.name.is_empty() {
        return Err(SpecToolError::InvalidRawFragment(
            "method fragments must include a method name".to_string(),
        ));
    }
    Ok(())
}
```

`method.name` is `ContractSpecString` which derefs to `Vec<u8>` which has `is_empty()`. No change needed.

- [ ] **Step 11: Update JSON rendering functions**

```rust
fn interface_to_json(interface: &ContractSpecInterface) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&interface.name),
        "version": spec_str(&interface.vers),
        "throws": interface.throws.iter().map(type_to_json).collect::<Vec<_>>(),
        "methods": interface.methods.iter().map(method_to_json).collect::<Vec<_>>(),
    })
}

fn method_to_json(method: &ContractSpecMethod) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&method.name),
        "args": method.args.iter().map(field_to_json).collect::<Vec<_>>(),
        "return_type": type_to_json(&method.return_type),
    })
}

fn field_to_json(field: &ContractField) -> serde_json::Value {
    serde_json::json!({
        "name": spec_str(&field.name),
        "ty": type_to_json(&field.ty),
    })
}
```

- [ ] **Step 12: Update `type_to_json`**

```rust
fn type_to_json(ty: &ContractType) -> serde_json::Value {
    use serde_json::json;
    match ty {
        ContractType::Void => json!({"kind": "void"}),
        ContractType::Bool => json!({"kind": "bool"}),
        ContractType::U8 => json!({"kind": "u8"}),
        ContractType::U32 => json!({"kind": "u32"}),
        ContractType::I32 => json!({"kind": "i32"}),
        ContractType::U64 => json!({"kind": "u64"}),
        ContractType::I64 => json!({"kind": "i64"}),
        ContractType::U128 => json!({"kind": "u128"}),
        ContractType::I128 => json!({"kind": "i128"}),
        ContractType::String => json!({"kind": "string"}),
        ContractType::Bytes => json!({"kind": "bytes"}),
        ContractType::Address => json!({"kind": "address"}),
        ContractType::Option { option_inner } => {
            let inner = option_inner.as_ref().map(|i| type_to_json(i)).unwrap_or(json!(null));
            json!({"kind": "option", "inner": inner})
        }
        ContractType::Vector { vector } => {
            let inner = vector.element.as_ref().map(|e| type_to_json(e)).unwrap_or(json!(null));
            json!({"kind": "vec", "inner": inner, "max_len": vector.max_len})
        }
        ContractType::Tuple { tuple } => {
            json!({"kind": "tuple", "items": tuple.iter().map(type_to_json).collect::<Vec<_>>()})
        }
        ContractType::Named { named } => json!({"kind": "named", "name": spec_str(named)}),
        ContractType::Array { array } => {
            let element = array.element.as_ref().map(|e| type_to_json(e)).unwrap_or(json!(null));
            json!({"kind": "array", "len": array.len, "element": element})
        }
        _ => json!({"kind": "unknown"}),
    }
}
```

- [ ] **Step 13: Update `type_def_to_json` and `union_case_type_to_json`**

```rust
fn type_def_to_json(type_def: &ContractTypeDef) -> serde_json::Value {
    use serde_json::json;
    match type_def {
        ContractTypeDef::TypeDefStruct { struct_def: def } => json!({
            "kind": "struct",
            "name": spec_str(&def.name),
            "fields": def.fields.iter().map(field_to_json).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefEnum { enum_def: def } => json!({
            "kind": "enum",
            "name": spec_str(&def.name),
            "variants": def.variants.iter().map(|v| json!({"name": spec_str(&v.name), "value": v.value})).collect::<Vec<_>>(),
        }),
        ContractTypeDef::TypeDefUnion { union_def: def } => json!({
            "kind": "union",
            "name": spec_str(&def.name),
            "cases": def.cases.iter().map(|c| json!({
                "name": spec_str(&c.name),
                "discriminant": c.discriminant,
                "case_type": union_case_type_to_json(&c.case_type),
            })).collect::<Vec<_>>(),
        }),
        _ => serde_json::json!({"kind": "unknown"}),
    }
}

fn union_case_type_to_json(
    case_type: &ContractUnionCaseType,
) -> serde_json::Value {
    use serde_json::json;
    match case_type {
        ContractUnionCaseType::UnionCaseVoid => json!({"kind": "void"}),
        ContractUnionCaseType::UnionCaseTuple { tuple } => json!({
            "kind": "tuple",
            "items": tuple.iter().map(type_to_json).collect::<Vec<_>>(),
        }),
        ContractUnionCaseType::UnionCaseStruct { struct_fields } => json!({
            "kind": "struct",
            "fields": struct_fields.iter().map(field_to_json).collect::<Vec<_>>(),
        }),
        _ => json!({"kind": "unknown"}),
    }
}
```

- [ ] **Step 14: Verify spec-tool compiles**

Run: `cargo check -p groundhog-spec-tool 2>&1 | tail -10`
Expected: compiles (tests may not pass yet — addressed in Task 5)

- [ ] **Step 15: Commit**

```bash
git add groundhog-spec-tool/src/lib.rs
git commit -m "Update spec-tool canonicalization for XDR-generated type shapes"
```

---

### Task 5: Update spec-tool tests

**Files:**
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`
- Modify: `groundhog-spec-tool/tests/rewrite.rs`

All test code constructs spec types using the old variant names and `String`/`Vec` fields. Every constructor and match arm needs updating.

Key patterns to replace throughout both test files:

**Constructors:**
- `"text".into()` for string fields → `ContractSpecString::try_from("text").unwrap()`
- `vec![...]` for collection fields → `vec![...].try_into().unwrap()`
- `SpecFragment::Interface(...)` → `SpecFragment::SpecInterface { interface_fragment: ... }`
- `SpecFragment::Method(...)` → `SpecFragment::SpecMethod { method_fragment: ... }`
- `SpecFragment::TypeDef(...)` → `SpecFragment::SpecTypeDef { type_def: ... }`
- `ContractTypeDef::Struct(...)` → `ContractTypeDef::TypeDefStruct { struct_def: ... }`
- `ContractTypeDef::Enum(...)` → `ContractTypeDef::TypeDefEnum { enum_def: ... }`
- `ContractTypeDef::Union(...)` → `ContractTypeDef::TypeDefUnion { union_def: ... }`
- `ContractType::Named("Foo".into())` → `ContractType::Named { named: s("Foo") }`
- `ContractType::Vector(ContractVectorType { element: Box::new(...), max_len })` → `ContractType::Vector { vector: ContractVectorType { element: Some(Box::new(...)), max_len } }`
- `ContractType::Array(ContractArrayType { element: Box::new(...), len })` → `ContractType::Array { array: ContractArrayType { element: Some(Box::new(...)), len } }`
- `ContractUnionCaseType::Void` → `ContractUnionCaseType::UnionCaseVoid`
- `ContractUnionCaseType::Struct(vec![...])` → `ContractUnionCaseType::UnionCaseStruct { struct_fields: vec![...].try_into().unwrap() }`
- `ContractUnionCaseType::Tuple(vec![...])` → `ContractUnionCaseType::UnionCaseTuple { tuple: vec![...].try_into().unwrap() }`
- `ContractSpecInterface.version` → `.vers`

**Assertions:**
- `spec.interfaces[0].name == "Named"` → `spec_str(&spec.interfaces[0].name) == "Named"` (or compare bytes: `*spec.interfaces[0].name == *b"Named"`)
- `method.name.as_str()` → `spec_str(&method.name)`
- `def.name.as_str()` in match arms → `spec_str(&def.name)` (but need to destructure struct variants first)
- `interface.version.as_str()` → `spec_str(&interface.vers)`
- Error variant matches like `SpecToolError::MissingTypeDef(name) if name == "DemoError"` stay the same (errors still use `String`)

- [ ] **Step 1: Update `canonicalize.rs` imports and add helper**

Replace imports:
```rust
use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpecMethod,
    ContractSpecString, ContractStructDef, ContractType, ContractTypeDef,
    ContractUnionCase, ContractUnionCaseType, ContractUnionDef,
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec, parse_raw_fragments,
};
use xdrs::txs::encode;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn spec_str(value: &ContractSpecString) -> &str {
    std::str::from_utf8(value).unwrap()
}
```

- [ ] **Step 2: Update all test functions in `canonicalize.rs`**

Mechanically update every constructor and assertion. Example for the first test:
```rust
#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canonicalize_spec(&[
        SpecFragment::SpecMethod { method_fragment: SpecMethodFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            method: ContractSpecMethod {
                name: s("ping"),
                args: vec![ContractField {
                    name: s("request"),
                    ty: ContractType::Named { named: s("Payload") },
                }].try_into().unwrap(),
                return_type: ContractType::Named { named: s("Response") },
            },
        }},
        SpecFragment::SpecInterface { interface_fragment: SpecInterfaceFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            throws: vec![ContractType::Named { named: s("DemoError") }].try_into().unwrap(),
        }},
        SpecFragment::SpecMethod { method_fragment: SpecMethodFragment {
            interface_name: s("Named"),
            interface_version: s("0.1.0"),
            method: ContractSpecMethod {
                name: s("pong"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::Void,
            },
        }},
        SpecFragment::SpecTypeDef { type_def: ContractTypeDef::TypeDefEnum { enum_def: ContractEnumDef {
            name: s("DemoError"),
            variants: vec![ContractEnumVariant {
                name: s("Nope"),
                value: 7,
            }].try_into().unwrap(),
        }}},
        SpecFragment::SpecTypeDef { type_def: ContractTypeDef::TypeDefStruct { struct_def: ContractStructDef {
            name: s("Payload"),
            fields: vec![ContractField {
                name: s("count"),
                ty: ContractType::U32,
            }].try_into().unwrap(),
        }}},
        SpecFragment::SpecTypeDef { type_def: ContractTypeDef::TypeDefStruct { struct_def: ContractStructDef {
            name: s("Response"),
            fields: vec![ContractField {
                name: s("ok"),
                ty: ContractType::Bool,
            }].try_into().unwrap(),
        }}},
    ])
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec_str(&spec.interfaces[0].name), "Named");
    assert_eq!(spec_str(&spec.interfaces[0].vers), "0.1.0");
    assert_eq!(
        spec.interfaces[0]
            .methods
            .iter()
            .map(|method| spec_str(&method.name))
            .collect::<Vec<_>>(),
        vec!["ping", "pong"]
    );
    assert_eq!(
        spec.type_defs
            .iter()
            .map(|type_def| match type_def {
                ContractTypeDef::TypeDefStruct { struct_def } => spec_str(&struct_def.name),
                ContractTypeDef::TypeDefEnum { enum_def } => spec_str(&enum_def.name),
                ContractTypeDef::TypeDefUnion { union_def } => spec_str(&union_def.name),
                _ => panic!("unknown variant"),
            })
            .collect::<Vec<_>>(),
        vec!["DemoError", "Payload", "Response"]
    );
}
```

Apply the same pattern to all remaining tests in `canonicalize.rs`. Each test follows the same mechanical transformation. (The full updated test file should be written in its entirety — do not skip tests.)

- [ ] **Step 3: Update `rewrite.rs` imports and helpers**

```rust
use groundhog_contract_spec::{
    ContractArrayType, ContractField, ContractSpec, ContractSpecMethod,
    ContractSpecString, ContractType, ContractVectorType, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
// ... rest of imports unchanged

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn spec_str(value: &ContractSpecString) -> &str {
    std::str::from_utf8(value).unwrap()
}
```

- [ ] **Step 4: Update all test functions in `rewrite.rs`**

Apply the same mechanical transformation. Key differences from `canonicalize.rs`:
- `ContractType::Vector(ContractVectorType { element: Box::new(...), max_len })` → `ContractType::Vector { vector: ContractVectorType { element: Some(Box::new(...)), max_len } }`
- `ContractType::Array(ContractArrayType { element: Box::new(...), len })` → `ContractType::Array { array: ContractArrayType { element: Some(Box::new(...)), len } }`
- `interface.name.as_str()` → `spec_str(&interface.name)`
- `interface.version.as_str()` → `spec_str(&interface.vers)`
- The `rewrite_omits_final_section_when_raw_has_only_typedefs` test uses `groundhog_contract_spec::ContractTypeDef::Struct(...)` → `groundhog_contract_spec::ContractTypeDef::TypeDefStruct { struct_def: ... }`

- [ ] **Step 5: Run spec-tool tests**

Run: `cargo test -p groundhog-spec-tool 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add groundhog-spec-tool/tests/canonicalize.rs groundhog-spec-tool/tests/rewrite.rs
git commit -m "Update spec-tool tests for XDR-generated type shapes"
```

---

### Task 6: Full workspace verification

- [ ] **Step 1: Check the full workspace compiles**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: compiles cleanly (warnings OK)

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: all tests pass

- [ ] **Step 3: Check `no_std` / no-alloc compilation still works**

Run: `cargo check -p groundhog-contract-spec --no-default-features 2>&1 | tail -10`
Expected: compiles (only view types available, no `groundhog-spec` dep pulled in)

- [ ] **Step 4: Format**

Run: `cargo fmt --all`

- [ ] **Step 5: Final commit if any formatting changes**

```bash
git add -u && git commit -m "cargo fmt"
```
