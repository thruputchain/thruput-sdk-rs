# Contract Spec Format Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a full hand-written contract spec crate, extend raw spec fragment emission from interface-only records to full interface/method/type-def facts, and teach the post-link tool to assemble and embed canonical XDR `ContractSpec` payloads.

**Architecture:** Keep the existing two-phase pipeline: macro/codegen emits appendable raw fragments into `groundhogspecrawv0`, then `groundhog-spec-tool` parses, validates, canonicalizes, and rewrites one final `groundhogspecv0`. The new `groundhog-contract-spec` crate owns the canonical XDR model plus const-friendly borrowed view types so generated code can leverage Rust type resolution for named types without trying to const-build the full transitive graph.

**Tech Stack:** Rust 2024, `xdrs` derive/encoding, proc-macro codegen, `wasmparser`, `wasm-encoder`, custom wasm sections.

---

## File Structure

### New Crate

- Create: `groundhog-contract-spec/Cargo.toml`
- Create: `groundhog-contract-spec/src/lib.rs`
- Create: `groundhog-contract-spec/src/model.rs`
- Create: `groundhog-contract-spec/src/view.rs`
- Create: `groundhog-contract-spec/tests/roundtrip.rs`

Responsibilities:

- `groundhog-contract-spec/src/model.rs`: owning XDR-serializable `ContractSpec`, `ContractType`, and named type-def records
- `groundhog-contract-spec/src/view.rs`: const-friendly borrowed view types (`&'static str`, slices, no `String`/`Vec`) and conversion into the owning model
- `groundhog-contract-spec/src/lib.rs`: public exports and `ContractTypeSpec` trait definition for future manual/derived named type emission
- `groundhog-contract-spec/tests/roundtrip.rs`: XDR roundtrip tests and view-to-owned conversion coverage

### SDK / Codegen Plumbing

- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Modify: `src/internal/mod.rs`
- Modify: `gen/src/spec_reg.rs`
- Modify: `gen/src/contract.rs`
- Modify: `macros/src/contract.rs`

Responsibilities:

- root `Cargo.toml`: add the new crate to the workspace and workspace dependencies
- `src/lib.rs`: re-export the new crate/types so generated code can refer to them through `::sdk::...`
- `src/internal/mod.rs`: add const raw-fragment encoders beyond the existing interface-only helper
- `gen/src/contract.rs`: expose the trait/bare-impl metadata needed to emit interface-level `throws`
- `gen/src/spec_reg.rs`: emit interface, method, and manual type-def raw fragments into `groundhogspecrawv0`
- `macros/src/contract.rs`: wire the richer raw-spec generation into trait impls, bare impls, and free functions

### Spec Tool

- Modify: `groundhog-spec-tool/src/model.rs`
- Modify: `groundhog-spec-tool/src/raw.rs`
- Modify: `groundhog-spec-tool/src/lib.rs`
- Modify: `groundhog-spec-tool/src/rewrite.rs`
- Modify: `groundhog-spec-tool/src/main.rs`
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`
- Modify: `groundhog-spec-tool/tests/rewrite.rs`

Responsibilities:

- `model.rs`: raw fragment structs/enums plus canonical assembly helpers
- `raw.rs`: XDR fragment-stream encoding/decoding helpers and final-section constants
- `lib.rs`: full `ContractSpec` canonicalization, closure checking, and JSON/debug rendering
- `rewrite.rs`: rewrite wasm custom sections using XDR final payloads
- `main.rs`: preserve the current CLI but write the richer JSON/debug sidecar
- tests: missing-type, conflicting-def, dedup, rewrite, and rerun coverage

### End-To-End Coverage

- Create: `example-contracts/spec_named_types/Cargo.toml`
- Create: `example-contracts/spec_named_types/src/lib.rs`
- Create: `example-contracts/spec_named_types/src/implementation.rs`
- Create: `example-contracts/spec_named_types/tests/spec.rs`

Responsibilities:

- hand-write one named struct, one enum, and one union-style type with manual `ContractTypeSpec` impls
- expose them through a contract interface and free-function/bare-impl surface
- verify `eg_build spec_named_types` produces a final wasm spec and JSON sidecar that include named type defs

## Task 1: Add The New Spec Crate

**Files:**
- Create: `groundhog-contract-spec/Cargo.toml`
- Create: `groundhog-contract-spec/src/lib.rs`
- Create: `groundhog-contract-spec/src/model.rs`
- Create: `groundhog-contract-spec/src/view.rs`
- Create: `groundhog-contract-spec/tests/roundtrip.rs`
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write the failing workspace expectation**

Run:

```bash
cargo test -p groundhog-contract-spec --offline
```

Expected: FAIL with “package ID specification `groundhog-contract-spec` did not match any packages”.

- [ ] **Step 2: Add the workspace crate skeleton**

Create `groundhog-contract-spec/Cargo.toml`:

```toml
[package]
name = "groundhog-contract-spec"
authors.workspace = true
license.workspace = true
edition.workspace = true
version.workspace = true
publish.workspace = true
repository.workspace = true
homepage.workspace = true
keywords.workspace = true
categories.workspace = true
description = "Canonical contract spec model for Groundhog contracts"

[dependencies]
xdrs = { workspace = true, features = ["heapless"] }
thiserror = { workspace = true, default-features = false }
```

Update the root workspace in `Cargo.toml`:

```toml
[workspace.dependencies]
groundhog-contract-spec = { path = "groundhog-contract-spec", registry = "thruput", version = "=0.8.6" }
```

and add `"groundhog-contract-spec"` to the `[workspace].members` array next to
`"groundhog-event"` and `"groundhog-spec-tool"`.

- [ ] **Step 3: Add the owning XDR model and const-friendly view model**

Create `groundhog-contract-spec/src/model.rs` with the owning wire model:

```rust
use xdrs::{ReadXDR, WriteXDR};

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpec {
    pub interfaces: Vec<ContractSpecInterface>,
    pub type_defs: Vec<ContractTypeDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecInterface {
    pub name: String,
    pub version: String,
    pub throws: Vec<ContractType>,
    pub methods: Vec<ContractSpecMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractSpecMethod {
    pub name: String,
    pub args: Vec<ContractField>,
    pub return_type: ContractType,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub struct ContractField {
    pub name: String,
    pub ty: ContractType,
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub enum ContractType {
    Void,
    Bool,
    U32,
    I32,
    U64,
    I64,
    String,
    Bytes,
    Address,
    Option(Box<ContractType>),
    Vec(Box<ContractType>),
    Tuple(Vec<ContractType>),
    Named(String),
}
```

Create `groundhog-contract-spec/src/view.rs` with borrowed const-friendly mirrors:

```rust
use crate::model::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractStructDef,
    ContractType, ContractTypeDef, ContractUnionCase, ContractUnionDef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeView<'a> {
    Void,
    Bool,
    U32,
    I32,
    U64,
    I64,
    String,
    Bytes,
    Address,
    Option(&'a ContractTypeView<'a>),
    Vec(&'a ContractTypeView<'a>),
    Tuple(&'a [ContractTypeView<'a>]),
    Named(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractFieldView<'a> {
    pub name: &'a str,
    pub ty: ContractTypeView<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeDefView<'a> {
    Struct(ContractStructDefView<'a>),
    Enum(ContractEnumDefView<'a>),
    Union(ContractUnionDefView<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractStructDefView<'a> {
    pub name: &'a str,
    pub fields: &'a [ContractFieldView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEnumDefView<'a> {
    pub name: &'a str,
    pub variants: &'a [ContractEnumVariantView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEnumVariantView<'a> {
    pub name: &'a str,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractUnionDefView<'a> {
    pub name: &'a str,
    pub cases: &'a [ContractUnionCaseView<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractUnionCaseView<'a> {
    pub name: &'a str,
    pub discriminant: i32,
    pub case_type: Option<ContractTypeView<'a>>,
}

impl From<ContractTypeView<'_>> for ContractType {
    fn from(value: ContractTypeView<'_>) -> Self {
        match value {
            ContractTypeView::Void => ContractType::Void,
            ContractTypeView::Bool => ContractType::Bool,
            ContractTypeView::U32 => ContractType::U32,
            ContractTypeView::I32 => ContractType::I32,
            ContractTypeView::U64 => ContractType::U64,
            ContractTypeView::I64 => ContractType::I64,
            ContractTypeView::String => ContractType::String,
            ContractTypeView::Bytes => ContractType::Bytes,
            ContractTypeView::Address => ContractType::Address,
            ContractTypeView::Option(inner) => {
                ContractType::Option(Box::new((*inner).into()))
            }
            ContractTypeView::Vec(inner) => {
                ContractType::Vec(Box::new((*inner).into()))
            }
            ContractTypeView::Tuple(items) => {
                ContractType::Tuple(items.iter().copied().map(Into::into).collect())
            }
            ContractTypeView::Named(name) => ContractType::Named(name.to_string()),
        }
    }
}
```

- [ ] **Step 4: Add the crate root and re-export it from `sdk`**

Create `groundhog-contract-spec/src/lib.rs`:

```rust
pub mod model;
pub mod view;

pub use model::*;
pub use view::*;

pub trait ContractTypeSpec {
    const SYMBOL: &'static str;
    const TYPE_REF: view::ContractTypeView<'static>;
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}

#[derive(Debug, Clone, PartialEq, Eq, ReadXDR, WriteXDR)]
pub enum SpecFragment {
    Interface(SpecInterfaceFragment),
    Method(SpecMethodFragment),
    TypeDef(ContractTypeDef),
}
```

Update `src/lib.rs`:

```rust
pub use groundhog_contract_spec as contract_spec;
pub use groundhog_contract_spec as __contract_spec;
```

- [ ] **Step 5: Add roundtrip and conversion tests**

Create `groundhog-contract-spec/tests/roundtrip.rs`:

```rust
use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractSpec, ContractSpecInterface,
    ContractSpecMethod, ContractStructDef, ContractType, ContractTypeDef,
    ContractTypeView, ContractUnionCase, ContractUnionDef, ContractField,
};
use xdrs::cxs::{decode, encode};

#[test]
fn contract_spec_roundtrips_through_xdr() {
    let spec = ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: "Named".to_string(),
            version: "0.1.0".to_string(),
            throws: vec![ContractType::Named("DemoError".to_string())],
            methods: vec![ContractSpecMethod {
                name: "ping".to_string(),
                args: vec![ContractField {
                    name: "value".to_string(),
                    ty: ContractType::U32,
                }],
                return_type: ContractType::Void,
            }],
        }],
        type_defs: vec![ContractTypeDef::Enum(ContractEnumDef {
            name: "DemoError".to_string(),
            variants: vec![ContractEnumVariant {
                name: "Nope".to_string(),
                value: 7,
            }],
        })],
    };

    let encoded = encode(&spec).unwrap();
    let decoded: ContractSpec = decode(&encoded).unwrap();
    assert_eq!(decoded, spec);
}

#[test]
fn borrowed_views_convert_to_owned_types() {
    let borrowed = ContractTypeView::Tuple(&[
        ContractTypeView::U32,
        ContractTypeView::Named("Payload"),
    ]);
    assert_eq!(
        groundhog_contract_spec::ContractType::from(borrowed),
        ContractType::Tuple(vec![
            ContractType::U32,
            ContractType::Named("Payload".to_string()),
        ]),
    );
}
```

- [ ] **Step 6: Run the new crate tests**

Run:

```bash
cargo test -p groundhog-contract-spec --offline
```

Expected: PASS with the new roundtrip tests green.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/lib.rs groundhog-contract-spec
git commit -m "feat: add contract spec model crate"
```

## Task 2: Replace The Interface-Only Raw Model

**Files:**
- Modify: `groundhog-spec-tool/src/model.rs`
- Modify: `groundhog-spec-tool/src/raw.rs`
- Modify: `groundhog-spec-tool/src/lib.rs`
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`

- [ ] **Step 1: Write the failing canonicalization tests**

Add to `groundhog-spec-tool/tests/canonicalize.rs`:

```rust
use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractField, ContractSpecMethod,
    ContractType, ContractTypeDef, SpecFragment, SpecInterfaceFragment,
    SpecMethodFragment,
};
use groundhog_spec_tool::{
    SpecToolError, canonicalize_spec,
};

#[test]
fn canonicalize_spec_groups_methods_and_type_defs() {
    let spec = canonicalize_spec(
        "demo",
        &[
            SpecFragment::Interface(SpecInterfaceFragment {
                package_name: "demo".into(),
                interface_name: "Named".into(),
                interface_version: "0.1.0".into(),
                throws: vec![ContractType::Named("DemoError".into())],
            }),
            SpecFragment::Method(SpecMethodFragment {
                package_name: "demo".into(),
                interface_name: "Named".into(),
                interface_version: "0.1.0".into(),
                method: ContractSpecMethod {
                    name: "ping".into(),
                    args: vec![ContractField {
                        name: "value".into(),
                        ty: ContractType::U32,
                    }],
                    return_type: ContractType::Void,
                },
            }),
            SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
                name: "DemoError".into(),
                variants: vec![ContractEnumVariant {
                    name: "Nope".into(),
                    value: 7,
                }],
            })),
        ],
    )
    .unwrap();

    assert_eq!(spec.interfaces.len(), 1);
    assert_eq!(spec.interfaces[0].methods.len(), 1);
    assert_eq!(spec.type_defs.len(), 1);
}

#[test]
fn canonicalize_spec_rejects_missing_named_type_defs() {
    let err = canonicalize_spec(
        "demo",
        &[SpecFragment::Interface(SpecInterfaceFragment {
            package_name: "demo".into(),
            interface_name: "Named".into(),
            interface_version: "0.1.0".into(),
            throws: vec![ContractType::Named("Missing".into())],
        })],
    )
    .unwrap_err();

    assert!(matches!(err, SpecToolError::MissingTypeDef(_)));
}
```

- [ ] **Step 2: Expand the raw model from interface-only to full XDR fragment enums**

Update `groundhog-spec-tool/src/model.rs`:

```rust
pub use groundhog_contract_spec::{
    SpecFragment, SpecInterfaceFragment, SpecMethodFragment,
};
```

Also replace `SpecToolError::NoInterfaceFragments` with richer variants:

```rust
pub enum SpecToolError {
    NoSpecFragments,
    InvalidRawFragment(String),
    MissingTypeDef(String),
    ConflictingTypeDef(String),
    ConflictingInterfaceThrows(String),
    Wasm(String),
}
```

- [ ] **Step 3: Teach the raw parser to read a stream of XDR `SpecFragment` values**

Update `groundhog-spec-tool/src/raw.rs` so it decodes concatenated XDR
fragments:

```rust
use groundhog_contract_spec::SpecFragment;
use xdrs::ReadXDR;

pub fn parse_raw_fragments(
    mut bytes: &[u8],
) -> Result<Vec<SpecFragment>, SpecToolError> {
    let mut fragments = Vec::new();
    while !bytes.is_empty() {
        let before = bytes.len();
        let fragment = SpecFragment::read_xdr(&mut bytes)
            .map_err(|err| SpecToolError::InvalidRawFragment(err.to_string()))?;
        if bytes.len() == before {
            return Err(SpecToolError::InvalidRawFragment(
                "spec fragment parser made no progress".to_string(),
            ));
        }
        fragments.push(fragment);
    }
    Ok(fragments)
}
```

- [ ] **Step 4: Replace `canonicalize_interfaces` with `canonicalize_spec`**

Update `groundhog-spec-tool/src/lib.rs`:

```rust
use groundhog_contract_spec::{ContractSpec, ContractSpecInterface, ContractType, ContractTypeDef};

pub fn canonicalize_spec(
    package_name: &str,
    raw: &[SpecFragment],
) -> Result<ContractSpec, SpecToolError> {
    let mut interfaces = std::collections::BTreeMap::new();
    let mut type_defs = std::collections::BTreeMap::new();

    for fragment in raw {
        match fragment {
            SpecFragment::Interface(iface) if iface.package_name == package_name => {
                // upsert interface shell, check throws agreement
            }
            SpecFragment::Method(method) if method.package_name == package_name => {
                // push methods into interface bucket
            }
            SpecFragment::TypeDef(ty) => {
                // dedup by symbol or reject conflicting shapes
            }
            _ => {}
        }
    }

    let spec = ContractSpec {
        interfaces: interfaces.into_values().collect(),
        type_defs: type_defs.into_values().collect(),
    };
    ensure_named_type_closure(&spec)?;
    Ok(spec)
}
```

Implement `ensure_named_type_closure` by walking:

- interface throws
- method args
- method returns
- nested struct fields
- nested union case payloads

- [ ] **Step 5: Run the spec-tool canonicalization tests**

Run:

```bash
cargo test -p groundhog-spec-tool canonicalize --offline
```

Expected: PASS with the new full-spec canonicalization tests green.

- [ ] **Step 6: Commit**

```bash
git add groundhog-spec-tool/src/model.rs groundhog-spec-tool/src/raw.rs groundhog-spec-tool/src/lib.rs groundhog-spec-tool/tests/canonicalize.rs
git commit -m "feat: assemble full contract specs"
```

## Task 3: Rewrite Final Wasm Sections As XDR `ContractSpec`

**Files:**
- Modify: `groundhog-spec-tool/src/rewrite.rs`
- Modify: `groundhog-spec-tool/src/main.rs`
- Modify: `groundhog-spec-tool/tests/rewrite.rs`

- [ ] **Step 1: Write the failing rewrite test for XDR payloads**

Add to `groundhog-spec-tool/tests/rewrite.rs`:

```rust
use groundhog_contract_spec::ContractSpec;
use groundhog_spec_tool::rewrite::rewrite_wasm_with_spec;
use wasmparser::{Parser, Payload};
use xdrs::cxs::decode;

#[test]
fn rewrite_wasm_writes_xdr_contract_spec_section() {
    let wasm = fixture_module_with_raw_spec_fragments();
    let rewritten = rewrite_wasm_with_spec(&wasm, "demo").unwrap();

    let mut final_payload = None;
    for payload in Parser::new(0).parse_all(&rewritten.wasm) {
        let payload = payload.unwrap();
        if let Payload::CustomSection(section) = payload {
            if section.name() == "groundhogspecv0" {
                final_payload = Some(section.data().to_vec());
            }
        }
    }

    let decoded: ContractSpec = decode(&final_payload.unwrap()).unwrap();
    assert_eq!(decoded.interfaces.len(), 1);
}
```

- [ ] **Step 2: Rewrite wasm using canonical `ContractSpec` instead of interface text**

Update `groundhog-spec-tool/src/rewrite.rs`:

```rust
use groundhog_contract_spec::ContractSpec;
use xdrs::cxs::encode;

pub struct RewrittenWasm {
    pub wasm: Vec<u8>,
    pub spec: Option<ContractSpec>,
}

pub fn rewrite_wasm_with_spec(
    bytes: &[u8],
    package_name: &str,
) -> Result<RewrittenWasm, SpecToolError> {
    let raw_fragments = collect_raw_fragments(bytes)?;
    let spec = if raw_fragments.is_empty() {
        collect_final_spec(bytes)?
    } else {
        Some(canonicalize_spec(package_name, &raw_fragments)?)
    };

    // existing section-copy loop stays, but final section now writes XDR bytes
    if let Some(spec) = &spec {
        module.section(&CustomSection {
            name: FINAL_SECTION_NAME.into(),
            data: encode(spec).unwrap().into(),
        });
    }
}
```

Keep the existing leak guard and rerun behavior, but change rerun parsing to decode XDR `ContractSpec` from `groundhogspecv0` instead of the old line-oriented interface text.

- [ ] **Step 3: Update the CLI JSON/debug sidecar**

Update `groundhog-spec-tool/src/main.rs`:

```rust
use groundhog_spec_tool::{render_spec_json, rewrite::rewrite_wasm_with_spec};

let rewritten = rewrite_wasm_with_spec(&bytes, &package_name)?;
fs::write(&wasm_path, rewritten.wasm)?;
if let Some(spec) = rewritten.spec {
    fs::write(&json_out, render_spec_json(&spec))?;
} else {
    // delete sidecar if no owned fragments
}
```

Update `render_spec_json` in `groundhog-spec-tool/src/lib.rs` to emit a debug view shaped like:

```json
{
  "format": "gh_contract_spec_v1",
  "interfaces": [
    {
      "name": "Named",
      "version": "0.1.0",
      "throws": [{"kind": "named", "name": "DemoError"}],
      "methods": [
        {
          "name": "ping",
          "args": [{"name": "value", "ty": {"kind": "u32"}}],
          "return_type": {"kind": "void"}
        }
      ]
    }
  ],
  "type_defs": [
    {
      "kind": "enum",
      "name": "DemoError",
      "variants": [{"name": "Nope", "value": 7}]
    }
  ]
}
```

- [ ] **Step 4: Run the rewrite tests**

Run:

```bash
cargo test -p groundhog-spec-tool rewrite --offline
```

Expected: PASS with the final-section XDR test and the rerun-idempotence test both green.

- [ ] **Step 5: Commit**

```bash
git add groundhog-spec-tool/src/rewrite.rs groundhog-spec-tool/src/main.rs groundhog-spec-tool/tests/rewrite.rs groundhog-spec-tool/src/lib.rs
git commit -m "feat: emit xdr contract specs in wasm"
```

## Task 4: Emit Interface And Method Fragments From `#[contract]`

**Files:**
- Modify: `src/internal/mod.rs`
- Modify: `gen/src/contract.rs`
- Modify: `gen/src/spec_reg.rs`
- Modify: `macros/src/contract.rs`
- Test: `gen/src/spec_reg.rs`

- [ ] **Step 1: Write the failing codegen tests**

Extend `gen/src/spec_reg.rs` tests with expectations for:

```rust
#[test]
fn trait_impl_fragments_include_interface_and_method_records() {
    let item: syn::ItemImpl = syn::parse_quote! {
        impl named_interface_callee::NamedInterfaceCallee for Demo {
            fn add_one(x: u32) -> u32 { x + 1 }
        }
    };

    let tokens = generate_impl_raw_spec_fragments(
        &item,
        &crate::Config::from_list(&[]).unwrap(),
    )
    .unwrap()
    .to_string();

    assert!(tokens.contains("groundhogspecrawv0"));
    assert!(tokens.contains("method"));
    assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"));
}
```

Expected before implementation: FAIL because only interface records exist today.

- [ ] **Step 2: Add const helpers for rich raw fragments**

Extend `src/internal/mod.rs` with helpers alongside `const_spec_raw_interface_fragment_bytes`:

```rust
pub const fn const_xdr_spec_interface_fragment_bytes<
    const OUT: usize,
>(
    fragment: &::groundhog_contract_spec::SpecInterfaceFragmentView<'_>,
) -> [u8; OUT] {
    // serialize one SpecFragment::Interface as XDR
}

pub const fn const_xdr_spec_method_fragment_bytes<
    const OUT: usize,
>(
    fragment: &::groundhog_contract_spec::SpecMethodFragmentView<'_>,
) -> [u8; OUT] {
    // serialize one SpecFragment::Method as XDR
}

pub const fn const_xdr_spec_typedef_fragment_bytes<
    const OUT: usize,
>(
    fragment: &::groundhog_contract_spec::ContractTypeDefView<'_>,
) -> [u8; OUT] {
    // serialize one SpecFragment::TypeDef as XDR
}
```

These helpers are the const-XDR staging encoders. They produce the exact raw
bytes written into `groundhogspecrawv0`.

- [ ] **Step 3: Emit richer interface fragments**

Update `gen/src/contract.rs` to expose interface-level throws as tokenized `ContractType` values:

```rust
pub fn throws_spec_tokens(
    contract: &impl ContractSrc,
) -> syn::Result<Vec<TokenStream>> {
    contract
        .throws_list()
        .into_iter()
        .map(|thrown| {
            let ty = thrown.ty();
            Ok(quote! { <#ty as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_REF.into_owned() })
        })
        .collect()
}
```

Then update `gen/src/spec_reg.rs` to emit one interface fragment and one method fragment per exported method:

```rust
pub fn generate_impl_raw_spec_fragments(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let interface_fragment = generate_interface_fragment_tokens(
        quote! { env!("CARGO_PKG_NAME") },
        iface_name.clone(),
        iface_vers.clone(),
        throws_tokens,
    );
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                quote! { env!("CARGO_PKG_NAME") },
                iface_name.clone(),
                iface_vers.clone(),
                method,
            )
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
    })
}
```

For each method fragment, build a borrowed `SpecMethodFragmentView` and emit
its XDR bytes with a const encoder:

```rust
const FRAGMENT: ::sdk::__contract_spec::SpecMethodFragmentView<'static> =
    ::sdk::__contract_spec::SpecMethodFragmentView {
        package_name: env!("CARGO_PKG_NAME"),
        interface_name: INTERFACE_NAME,
        interface_version: INTERFACE_VERSION,
        method: ::sdk::__contract_spec::ContractSpecMethodView {
            name: #method_name,
            args: METHOD_ARGS,
            return_type: METHOD_RETURN,
        },
    };

const RAW_FRAGMENT: [u8; RAW_LEN] =
    ::sdk::internal::const_xdr_spec_method_fragment_bytes::<RAW_LEN>(&FRAGMENT);
```

Mirror that shape for interface fragments with
`const_xdr_spec_interface_fragment_bytes`, and for manual named types with
`const_xdr_spec_typedef_fragment_bytes(&<T as ContractTypeSpec>::TYPE_DEF)`.

- [ ] **Step 4: Wire the macro entrypoints to the new helpers**

Update `macros/src/contract.rs`:

```rust
out.extend(codegen::spec_reg::generate_impl_raw_spec_fragments(item, config)?);
out.extend(codegen::spec_reg::generate_free_function_raw_spec_fragments(item, config)?);
```

Do not change trait definitions to emit exported method fragments; trait definitions remain metadata sources only.

- [ ] **Step 5: Run the codegen tests**

Run:

```bash
cargo test -p groundhog-codegen spec_reg --offline
```

Expected: PASS with interface-and-method fragment coverage green.

- [ ] **Step 6: Commit**

```bash
git add src/internal/mod.rs gen/src/contract.rs gen/src/spec_reg.rs macros/src/contract.rs
git commit -m "feat: emit method spec fragments"
```

## Task 5: Add Manual Named Type Fragments And Closure Validation

**Files:**
- Modify: `groundhog-contract-spec/src/lib.rs`
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`
- Create: `example-contracts/spec_named_types/Cargo.toml`
- Create: `example-contracts/spec_named_types/src/lib.rs`
- Create: `example-contracts/spec_named_types/src/implementation.rs`
- Create: `example-contracts/spec_named_types/tests/spec.rs`

- [ ] **Step 1: Write the failing end-to-end named-type test**

Create `example-contracts/spec_named_types/tests/spec.rs`:

```rust
use std::fs;
use std::path::PathBuf;

#[test]
fn built_spec_includes_named_type_defs() {
    let sdk_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let json = sdk_dir.join("target/optimized/spec_named_types.spec.json");
    let text = fs::read_to_string(json).unwrap();

    assert!(text.contains("\"type_defs\""));
    assert!(text.contains("\"Payload\""));
    assert!(text.contains("\"Choice\""));
    assert!(text.contains("\"Envelope\""));
}
```

Expected before implementation: FAIL because no named type defs are emitted yet.

- [ ] **Step 2: Add a manual `ContractTypeSpec` example crate**

Create `example-contracts/spec_named_types/src/lib.rs`:

```rust
#![cfg_attr(target_family = "wasm", no_std)]

use sdk::contract;

mod implementation;

pub use implementation::*;
```

Create `example-contracts/spec_named_types/src/implementation.rs`:

```rust
use sdk::__contract_spec::{
    ContractEnumDefView, ContractEnumVariantView, ContractFieldView,
    ContractStructDefView, ContractTypeDefView, ContractTypeSpec,
    ContractTypeView, ContractUnionCaseView, ContractUnionDefView,
};

pub struct Payload {
    pub count: u32,
}

pub enum Choice {
    Left = 1,
    Right = 2,
}

pub enum Envelope {
    Empty,
    Wrapped(Payload),
}

impl ContractTypeSpec for Payload {
    const SYMBOL: &'static str = "Payload";
    const TYPE_REF: ContractTypeView<'static> = ContractTypeView::Named("Payload");
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Struct(ContractStructDefView {
            name: "Payload",
            fields: &[ContractFieldView {
                name: "count",
                ty: ContractTypeView::U32,
            }],
        });
}

impl ContractTypeSpec for Choice {
    const SYMBOL: &'static str = "Choice";
    const TYPE_REF: ContractTypeView<'static> = ContractTypeView::Named("Choice");
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Enum(ContractEnumDefView {
            name: "Choice",
            variants: &[
                ContractEnumVariantView { name: "Left", value: 1 },
                ContractEnumVariantView { name: "Right", value: 2 },
            ],
        });
}

impl ContractTypeSpec for Envelope {
    const SYMBOL: &'static str = "Envelope";
    const TYPE_REF: ContractTypeView<'static> = ContractTypeView::Named("Envelope");
    const TYPE_DEF: ContractTypeDefView<'static> =
        ContractTypeDefView::Union(ContractUnionDefView {
            name: "Envelope",
            cases: &[
                ContractUnionCaseView {
                    name: "Empty",
                    discriminant: 0,
                    case_type: None,
                },
                ContractUnionCaseView {
                    name: "Wrapped",
                    discriminant: 1,
                    case_type: Some(ContractTypeView::Named("Payload")),
                },
            ],
        });
}

#[contract]
impl SpecNamedTypes {
    pub fn echo(payload: Payload) -> Payload { payload }
    pub fn choose(choice: Choice) -> Choice { choice }
    pub fn wrap(flag: bool, payload: Payload) -> Envelope {
        if flag { Envelope::Wrapped(payload) } else { Envelope::Empty }
    }
}
```

- [ ] **Step 3: Teach the postprocessor to reject missing or conflicting named defs**

Add/extend `groundhog-spec-tool/tests/canonicalize.rs`:

```rust
#[test]
fn canonicalize_spec_rejects_conflicting_type_defs() {
    let left = SpecFragment::TypeDef(ContractTypeDef::Struct(ContractStructDef {
        name: "Payload".into(),
        fields: vec![],
    }));
    let right = SpecFragment::TypeDef(ContractTypeDef::Enum(ContractEnumDef {
        name: "Payload".into(),
        variants: vec![],
    }));

    let err = canonicalize_spec("demo", &[left, right]).unwrap_err();
    assert!(matches!(err, SpecToolError::ConflictingTypeDef(name) if name == "Payload"));
}
```

Implement the corresponding conflict check in `groundhog-spec-tool/src/lib.rs` by keying `type_defs` on symbol and comparing normalized owning `ContractTypeDef` values.

- [ ] **Step 4: Build the example and verify the sidecar**

Run:

```bash
source scripts/dev.sh
eg_build spec_named_types
cargo test --manifest-path example-contracts/Cargo.toml -p spec_named_types --test spec -- --show-output
```

Expected:

- `eg_build` succeeds
- `target/optimized/spec_named_types.spec.json` exists
- the test sees `Payload`, `Choice`, and `Envelope` in `type_defs`

- [ ] **Step 5: Commit**

```bash
git add groundhog-contract-spec/src/lib.rs groundhog-spec-tool/tests/canonicalize.rs example-contracts/spec_named_types
git commit -m "test: cover named type spec fragments"
```

## Task 6: Full Verification Sweep

**Files:**
- Modify: `groundhog-spec-tool/tests/rewrite.rs`
- Modify: `example-contracts/spec_named_types/tests/spec.rs`

- [ ] **Step 1: Run the focused Rust test suites**

Run:

```bash
cargo test -p groundhog-contract-spec --offline
cargo test -p groundhog-spec-tool --offline
cargo test -p groundhog-codegen spec_reg --offline
```

Expected: PASS for all three commands.

- [ ] **Step 2: Run the end-to-end example build**

Run:

```bash
source scripts/dev.sh
eg_build spec_named_types
eg_build free_fn_unnamed
eg_build multi_imported_traits
```

Expected:

- all three builds succeed
- `spec_named_types.spec.json` contains named type defs
- `free_fn_unnamed.spec.json` still contains one unnamed interface bucket
- `multi_imported_traits.spec.json` still contains the imported interfaces

- [ ] **Step 3: Inspect final wasm sections**

Run:

```bash
wasm-objdump -x target/optimized/spec_named_types.wasm | rg 'groundhogspec'
wasm-objdump -x target/optimized/free_fn_unnamed.wasm | rg 'groundhogspec'
```

Expected:

- only `groundhogspecv0` appears
- `groundhogspecrawv0` is absent from finalized artifacts

- [ ] **Step 4: Commit any last test-only adjustments**

```bash
git add groundhog-spec-tool/tests example-contracts/spec_named_types/tests
git commit -m "test: verify full contract spec pipeline"
```

## Self-Review

### Spec coverage

- Full hand-written spec crate: Task 1
- Full contract spec model with interfaces, methods, throws, and named type defs: Tasks 1 and 2
- Const-resolved local type reflection for named types: Tasks 1 and 5
- Raw fragment expansion beyond interfaces: Tasks 2 and 4
- Post-link canonicalization and closure checking: Tasks 2 and 3
- Final XDR section and JSON/debug artifact: Task 3
- Manual named-type coverage before derive macros: Task 5
- End-to-end wasm/spec verification: Task 6

### Placeholder scan

Checked common placeholder markers and vague fallback phrasing. None remain in
the task steps.

### Type consistency

- Canonical owning model is `ContractSpec` / `ContractType`
- Const-friendly reflection model is `ContractTypeView<'a>` and companion view records
- Tool assembly entrypoint is `canonicalize_spec`
- Rewrite entrypoint is `rewrite_wasm_with_spec`

These names are used consistently across the plan.
