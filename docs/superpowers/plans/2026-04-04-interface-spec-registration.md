# Interface Spec Registration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit raw interface-spec fragments into contract wasm at link time, run a postprocessing pass that rewrites one canonical final custom section plus a JSON sidecar, and verify the flow for trait impls, bare impls, named free functions, and unnamed free functions.

**Architecture:** Proc-macro/codegen changes emit one raw interface fragment per exported contract surface into a staging wasm custom section, `groundhogspecrawv0`. A new post-link tool crate parses those fragments, validates crate-level invariants, merges and dedupes interfaces, rewrites the wasm with one final `groundhogspecv0` section, and optionally emits a sidecar JSON file. The existing build script becomes the blessed path that invokes this rewrite automatically after the wasm is optimized.

**Tech Stack:** Rust proc macros (`macros`, `gen`), SDK hidden wasm metadata, `wasmparser`/`wasm-encoder` in a new tool crate, shell build integration in `scripts/dev.sh`, example-contract wasm artifact tests.

---

## File Map

- Create: `groundhog-spec-tool/Cargo.toml`
- Create: `groundhog-spec-tool/src/lib.rs`
- Create: `groundhog-spec-tool/src/model.rs`
- Create: `groundhog-spec-tool/src/raw.rs`
- Create: `groundhog-spec-tool/src/rewrite.rs`
- Create: `groundhog-spec-tool/src/main.rs`
- Create: `groundhog-spec-tool/tests/canonicalize.rs`
- Create: `groundhog-spec-tool/tests/rewrite.rs`
- Create: `example-contracts/free_fn_unnamed/Cargo.toml`
- Create: `example-contracts/free_fn_unnamed/src/lib.rs`
- Create: `example-contracts/free_fn_unnamed/src/implementation.rs`
- Modify: `Cargo.toml`
- Modify: `example-contracts/Cargo.toml`
- Modify: `gen/src/lib.rs`
- Create: `gen/src/spec_reg.rs`
- Modify: `macros/src/contract.rs`
- Modify: `src/lib.rs`
- Modify: `src/internal/mod.rs`
- Modify: `scripts/dev.sh`
- Test: `groundhog-spec-tool/tests/canonicalize.rs`
- Test: `groundhog-spec-tool/tests/rewrite.rs`

### Responsibility Split

- `groundhog-spec-tool/src/model.rs` owns the in-memory raw and canonical interface records.
- `groundhog-spec-tool/src/raw.rs` owns the dumb inspectable raw fragment text format.
- `groundhog-spec-tool/src/rewrite.rs` owns wasm parsing, validation, merge/dedupe, and final-section rewrite.
- `groundhog-spec-tool/src/main.rs` exposes the CLI used by the build script.
- `gen/src/spec_reg.rs` owns raw fragment code generation for trait impls, bare impls, and free functions.
- `macros/src/contract.rs` wires spec fragment emission into all export-capable `#[contract]` items.
- `src/lib.rs` owns any hidden wasm anchor needed to keep custom sections retained in final contract artifacts.
- `scripts/dev.sh` becomes the blessed repo build path that always rewrites optimized contract wasm files with the final section and JSON sidecar.
- `example-contracts/free_fn_unnamed` is the minimal free-function-only fixture for unnamed-interface coverage.

### Canonical Constants

Use these constants across the implementation:

```rust
pub const RAW_SECTION_NAME: &str = "groundhogspecrawv0";
pub const FINAL_SECTION_NAME: &str = "groundhogspecv0";
pub const RAW_RECORD_TAG: &str = "iface";
pub const FINAL_FORMAT_TAG: &str = "gh_iface_v1";
pub const UNNAMED_INTERFACE: &str = "_unnamed_interface";
```

### JSON Sidecar Path

Write the convenience artifact next to the optimized wasm:

```text
target/optimized/<contract>.wasm
target/optimized/<contract>.spec.json
```

### CLI Shape

The build hook should call a simple CLI:

```bash
cargo run -p groundhog-spec-tool -- \
  rewrite \
  --wasm target/optimized/<contract>.wasm \
  --json-out target/optimized/<contract>.spec.json
```

## Task 1: Create The Spec Tool Crate And Canonicalization Core

**Files:**
- Create: `groundhog-spec-tool/Cargo.toml`
- Create: `groundhog-spec-tool/src/lib.rs`
- Create: `groundhog-spec-tool/src/model.rs`
- Create: `groundhog-spec-tool/src/raw.rs`
- Test: `groundhog-spec-tool/tests/canonicalize.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add a failing canonicalization test suite**

Create `groundhog-spec-tool/tests/canonicalize.rs`:

```rust
use groundhog_spec_tool::{
    canonicalize_interfaces, InterfaceId, RawInterfaceFragment,
};

#[test]
fn merges_duplicate_free_function_interfaces() {
    let raw = vec![
        RawInterfaceFragment::free_fn(
            "erc20_low_level",
            "ERC20",
            "0.1.0",
        ),
        RawInterfaceFragment::free_fn(
            "erc20_low_level",
            "ERC20",
            "0.1.0",
        ),
    ];

    let interfaces = canonicalize_interfaces(&raw).unwrap();
    assert_eq!(
        interfaces,
        vec![InterfaceId::new("ERC20", "0.1.0")],
    );
}

#[test]
fn rejects_multiple_impl_backed_self_types() {
    let raw = vec![
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "Alpha",
            "0.3.0",
        ),
        RawInterfaceFragment::impl_backed(
            "demo",
            "Beta",
            "Beta",
            "0.3.0",
        ),
    ];

    let err = canonicalize_interfaces(&raw).unwrap_err();
    assert!(err.to_string().contains("multiple impl-backed self types"));
}

#[test]
fn rejects_empty_interface_sets() {
    let raw = vec![];
    let err = canonicalize_interfaces(&raw).unwrap_err();
    assert!(err.to_string().contains("no interface fragments"));
}
```

- [ ] **Step 2: Run the new test target and verify it fails**

Run:

```bash
cargo test -p groundhog-spec-tool --test canonicalize --offline
```

Expected: FAIL because the crate and symbols do not exist yet.

- [ ] **Step 3: Add the tool crate, models, and canonicalization logic**

Create `groundhog-spec-tool/Cargo.toml`:

```toml
[package]
name = "groundhog-spec-tool"
edition.workspace = true
version.workspace = true
authors.workspace = true
license.workspace = true
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror.workspace = true
```

Create `groundhog-spec-tool/src/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InterfaceId {
    pub interface_name: String,
    pub interface_version: String,
}

impl InterfaceId {
    pub fn new(interface_name: impl Into<String>, interface_version: impl Into<String>) -> Self {
        Self {
            interface_name: interface_name.into(),
            interface_version: interface_version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawInterfaceFragment {
    pub package_name: String,
    pub self_type_key: Option<String>,
    pub interface_name: String,
    pub interface_version: String,
}

impl RawInterfaceFragment {
    pub fn free_fn(package: &str, interface_name: &str, interface_version: &str) -> Self {
        Self {
            package_name: package.to_owned(),
            self_type_key: None,
            interface_name: interface_name.to_owned(),
            interface_version: interface_version.to_owned(),
        }
    }

    pub fn impl_backed(
        package: &str,
        self_type_key: &str,
        interface_name: &str,
        interface_version: &str,
    ) -> Self {
        Self {
            package_name: package.to_owned(),
            self_type_key: Some(self_type_key.to_owned()),
            interface_name: interface_name.to_owned(),
            interface_version: interface_version.to_owned(),
        }
    }
}
```

Create `groundhog-spec-tool/src/lib.rs`:

```rust
mod model;
pub mod raw;
pub mod rewrite;

pub use model::{InterfaceId, RawInterfaceFragment};

#[derive(Debug, thiserror::Error)]
pub enum SpecToolError {
    #[error("no interface fragments found")]
    NoInterfaceFragments,
    #[error("multiple impl-backed self types found: `{0}` and `{1}`")]
    MultipleImplTypes(String, String),
    #[error("invalid raw interface fragment: {0}")]
    InvalidRawFragment(String),
    #[error("wasm rewrite error: {0}")]
    Wasm(String),
}

pub fn canonicalize_interfaces(
    raw: &[RawInterfaceFragment],
) -> Result<Vec<InterfaceId>, SpecToolError> {
    if raw.is_empty() {
        return Err(SpecToolError::NoInterfaceFragments);
    }

    let mut seen_impl = None::<String>;
    for fragment in raw {
        if let Some(key) = &fragment.self_type_key {
            match &seen_impl {
                None => seen_impl = Some(key.clone()),
                Some(existing) if existing == key => {}
                Some(existing) => {
                    return Err(SpecToolError::MultipleImplTypes(
                        existing.clone(),
                        key.clone(),
                    ));
                }
            }
        }
    }

    let mut interfaces = raw
        .iter()
        .map(|fragment| {
            InterfaceId::new(
                fragment.interface_name.clone(),
                fragment.interface_version.clone(),
            )
        })
        .collect::<Vec<_>>();
    interfaces.sort();
    interfaces.dedup();
    Ok(interfaces)
}
```

- [ ] **Step 4: Run the canonicalization tests and make sure they pass**

Run:

```bash
cargo test -p groundhog-spec-tool --test canonicalize --offline
```

Expected: PASS with 3 passing tests.

- [ ] **Step 5: Commit the canonicalization core**

```bash
git add Cargo.toml groundhog-spec-tool
git commit -m "feat: add interface spec tool core"
```

## Task 2: Add Raw Interface Fragment Encoding And Rewrite Tests

**Files:**
- Create: `groundhog-spec-tool/src/raw.rs`
- Create: `groundhog-spec-tool/src/rewrite.rs`
- Create: `groundhog-spec-tool/tests/rewrite.rs`
- Modify: `groundhog-spec-tool/Cargo.toml`

- [ ] **Step 1: Add a failing wasm rewrite integration test**

Create `groundhog-spec-tool/tests/rewrite.rs`:

```rust
use groundhog_spec_tool::{rewrite::rewrite_wasm, FINAL_SECTION_NAME, RAW_SECTION_NAME};
use wasm_encoder::{CustomSection, Module};
use wasmparser::{Parser, Payload};

fn custom_sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut sections = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            sections.push((section.name().to_owned(), section.data().to_vec()));
        }
    }
    sections
}

#[test]
fn rewrite_replaces_raw_section_with_one_final_section() {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: b"iface\tdemo\tAlpha\tExplicitNamedInterface\t0.3.0\n\
                iface\tdemo\tAlpha\tVersionedCallee\t1.2.3\n"
            .into(),
    });

    let rewritten = rewrite_wasm(&module.finish()).unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    let final_sections: Vec<_> = sections
        .into_iter()
        .filter(|(name, _)| name == FINAL_SECTION_NAME)
        .collect();
    assert_eq!(final_sections.len(), 1);
    let body = String::from_utf8(final_sections[0].1.clone()).unwrap();
    assert!(body.contains("gh_iface_v1"));
    assert!(body.contains("ExplicitNamedInterface\t0.3.0"));
    assert!(body.contains("VersionedCallee\t1.2.3"));
}
```

- [ ] **Step 2: Run the rewrite test and verify it fails**

Run:

```bash
cargo test -p groundhog-spec-tool --test rewrite --offline
```

Expected: FAIL because `rewrite_wasm`, section constants, and wasm dependencies are not implemented.

- [ ] **Step 3: Implement raw encoding constants and wasm rewrite logic**

Update `groundhog-spec-tool/Cargo.toml` dependencies:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror.workspace = true
wasm-encoder = "0.240"
wasmparser = "0.240"
```

Create `groundhog-spec-tool/src/raw.rs`:

```rust
use crate::{RawInterfaceFragment, SpecToolError};

pub const RAW_SECTION_NAME: &str = "groundhogspecrawv0";
pub const FINAL_SECTION_NAME: &str = "groundhogspecv0";
pub const RAW_RECORD_TAG: &str = "iface";
pub const FINAL_FORMAT_TAG: &str = "gh_iface_v1";

pub fn encode_raw_fragment(fragment: &RawInterfaceFragment) -> Vec<u8> {
    format!(
        "{RAW_RECORD_TAG}\t{}\t{}\t{}\t{}\n",
        fragment.package_name,
        fragment.self_type_key.as_deref().unwrap_or(""),
        fragment.interface_name,
        fragment.interface_version,
    )
    .into_bytes()
}

pub fn parse_raw_fragment_line(line: &str) -> Result<RawInterfaceFragment, SpecToolError> {
    let parts = line.split('\t').collect::<Vec<_>>();
    match parts.as_slice() {
        ["iface", package_name, self_type_key, interface_name, interface_version] => {
            Ok(RawInterfaceFragment {
                package_name: (*package_name).to_owned(),
                self_type_key: if self_type_key.is_empty() {
                    None
                } else {
                    Some((*self_type_key).to_owned())
                },
                interface_name: (*interface_name).to_owned(),
                interface_version: (*interface_version).to_owned(),
            })
        }
        _ => Err(SpecToolError::InvalidRawFragment(line.to_owned())),
    }
}
```

Create `groundhog-spec-tool/src/rewrite.rs`:

```rust
use wasm_encoder::{CustomSection, Module, RawSection};
use wasmparser::{Parser, Payload};

use crate::{
    canonicalize_interfaces,
    raw::{parse_raw_fragment_line, FINAL_FORMAT_TAG, FINAL_SECTION_NAME, RAW_SECTION_NAME},
    RawInterfaceFragment, SpecToolError,
};

pub fn rewrite_wasm(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    let mut raw_fragments = Vec::<RawInterfaceFragment>::new();
    let mut module = Module::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|e| SpecToolError::Wasm(e.to_string()))? {
            Payload::Version { num, encoding, range } => {
                module.section(&RawSection {
                    id: wasm_encoder::SectionId::Custom as u8 - 1,
                    data: &bytes[range.start..range.end],
                });
                let _ = (num, encoding);
            }
            Payload::CustomSection(section) if section.name() == RAW_SECTION_NAME => {
                let text = core::str::from_utf8(section.data())
                    .map_err(|e| SpecToolError::Wasm(e.to_string()))?;
                for line in text.lines().filter(|line| !line.is_empty()) {
                    raw_fragments.push(parse_raw_fragment_line(line)?);
                }
            }
            Payload::CustomSection(section) => {
                module.section(&CustomSection {
                    name: section.name().into(),
                    data: section.data().into(),
                });
            }
            other => {
                if let Some((id, range)) = section_range(&other) {
                    module.section(&RawSection {
                        id,
                        data: &bytes[range.start..range.end],
                    });
                }
            }
        }
    }

    let interfaces = canonicalize_interfaces(&raw_fragments)?;
    let mut final_text = String::from(FINAL_FORMAT_TAG);
    final_text.push('\n');
    for interface in interfaces {
        final_text.push_str(&interface.interface_name);
        final_text.push('\t');
        final_text.push_str(&interface.interface_version);
        final_text.push('\n');
    }

    module.section(&CustomSection {
        name: FINAL_SECTION_NAME.into(),
        data: final_text.into_bytes().into(),
    });
    Ok(module.finish())
}

fn push_section(
    module: &mut Module,
    bytes: &[u8],
    id: u8,
    range: core::ops::Range<usize>,
) {
    module.section(&RawSection {
        id,
        data: &bytes[range.start..range.end],
    });
}
```

- [ ] **Step 4: Run both tool test targets and verify they pass**

Run:

```bash
cargo test -p groundhog-spec-tool --test canonicalize --test rewrite --offline
```

Expected: PASS with both test targets green.

- [ ] **Step 5: Commit the raw-section and rewrite implementation**

```bash
git add groundhog-spec-tool
git commit -m "feat: add interface spec wasm rewrite tool"
```

## Task 3: Emit Raw Interface Fragments From Contract Macro Expansions

**Files:**
- Create: `gen/src/spec_reg.rs`
- Modify: `gen/src/lib.rs`
- Modify: `macros/src/contract.rs`
- Modify: `src/lib.rs`
- Modify: `src/internal/mod.rs`

- [ ] **Step 1: Add failing codegen tests for trait impls and free functions**

Create unit tests in `gen/src/spec_reg.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_impl_fragments_use_trait_metadata_consts() {
        let item: syn::ItemImpl = syn::parse_quote! {
            impl named_interface_callee::NamedInterfaceCallee for Demo {
                fn add_one(x: u32) -> u32 { x + 1 }
            }
        };
        let tokens = generate_impl_raw_spec_fragment(&item, &crate::Config::from_list(&[]).unwrap())
            .unwrap()
            .to_string();

        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"));
        assert!(tokens.contains("__GROUNDHOG_IFACE_FULL_VERSION"));
        assert!(tokens.contains("groundhogspecrawv0"));
    }

    #[test]
    fn free_function_fragments_use_effective_interface_name() {
        let item: syn::ItemFn = syn::parse_quote! {
            fn transfer() {}
        };
        let config = crate::Config::from_list(
            &darling::ast::NestedMeta::parse_meta_list(
                "interface = \"ERC20\", version = \"0.1.0\"".parse().unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        let tokens =
            generate_free_function_raw_spec_fragment(&item, &config).unwrap().to_string();
        assert!(tokens.contains("groundhogspecrawv0"));
        assert!(tokens.contains("ERC20"));
        assert!(tokens.contains("0.1.0"));
    }
}
```

- [ ] **Step 2: Run the codegen unit tests and verify they fail**

Run:

```bash
cargo test -p groundhog-codegen spec_reg --offline
```

Expected: FAIL because the module and generators do not exist yet.

- [ ] **Step 3: Implement raw fragment emission and macro wiring**

Create `gen/src/spec_reg.rs`:

```rust
use proc_macro2::Literal;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Result;

use crate::{Config, ConfiguredContract, ContractSrc};

pub fn generate_impl_raw_spec_fragment(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let self_ty = &item.self_ty;
    let self_type_key = quote!(stringify!(#self_ty));

    let (iface_name_expr, iface_version_expr) = if let Some((_, trait_path, _)) = &item.trait_ {
        (
            quote!(<#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME),
            quote!(<#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION),
        )
    } else {
        let contract = ConfiguredContract::new(config.clone(), item);
        let iface_name = crate::contract::effective_interface_name(&contract);
        let iface_version = contract.defaulted_interface_vers();
        (quote!(#iface_name), quote!(#iface_version))
    };

    Ok(generate_raw_fragment_tokens(
        quote!(env!("CARGO_PKG_NAME")),
        self_type_key,
        iface_name_expr,
        iface_version_expr,
    ))
}

pub fn generate_free_function_raw_spec_fragment(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = crate::contract::effective_interface_name(&contract);
    let iface_version = contract.defaulted_interface_vers();

    Ok(generate_raw_fragment_tokens(
        quote!(env!("CARGO_PKG_NAME")),
        quote!(""),
        quote!(#iface_name),
        quote!(#iface_version),
    ))
}

fn generate_raw_fragment_tokens(
    package_name: TokenStream,
    self_type_key: TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
) -> TokenStream {
    quote! {
        #[cfg(target_family = "wasm")]
        const _: () = {
            const PACKAGE_NAME: &str = #package_name;
            const SELF_TYPE_KEY: &str = #self_type_key;
            const INTERFACE_NAME: &str = #interface_name;
            const INTERFACE_VERSION: &str = #interface_version;
            const RAW_FRAGMENT: [u8; {
                10 + PACKAGE_NAME.len()
                    + SELF_TYPE_KEY.len()
                    + INTERFACE_NAME.len()
                    + INTERFACE_VERSION.len()
            }] = ::sdk::internal::const_spec_raw_interface_fragment_bytes::<
                { PACKAGE_NAME.len() },
                { SELF_TYPE_KEY.len() },
                { INTERFACE_NAME.len() },
                { INTERFACE_VERSION.len() },
                {
                    10 + PACKAGE_NAME.len()
                        + SELF_TYPE_KEY.len()
                        + INTERFACE_NAME.len()
                        + INTERFACE_VERSION.len()
                },
            >(PACKAGE_NAME, SELF_TYPE_KEY, INTERFACE_NAME, INTERFACE_VERSION);

            #[used]
            #[link_section = "groundhogspecrawv0"]
            static RAW_INTERFACE_SPEC: [u8; RAW_FRAGMENT.len()] = RAW_FRAGMENT;
        };
    }
}
```

Then wire it up:

`gen/src/lib.rs`

```rust
pub mod spec_reg;
```

`macros/src/contract.rs`

```rust
out.extend(codegen::spec_reg::generate_impl_raw_spec_fragment(item, config)?);
```

for impls, and:

```rust
out.extend(codegen::spec_reg::generate_free_function_raw_spec_fragment(item, config)?);
```

for free functions.

Add a hidden wasm retention anchor in `src/lib.rs`:

```rust
#[cfg(target_family = "wasm")]
const _: () = {
    #[export_name = "_"]
    static __GROUNDHOG_SPEC_ANCHOR: () = ();
};
```

Add the compile-time raw-fragment builder to `src/internal/mod.rs`:

```rust
#[doc(hidden)]
#[inline]
pub const fn const_spec_raw_interface_fragment_bytes<
    const P: usize,
    const S: usize,
    const I: usize,
    const V: usize,
    const OUT: usize,
>(
    package_name: &str,
    self_type_key: &str,
    interface_name: &str,
    interface_version: &str,
) -> [u8; OUT] {
    assert!(package_name.len() == P);
    assert!(self_type_key.len() == S);
    assert!(interface_name.len() == I);
    assert!(interface_version.len() == V);
    assert!(OUT == 10 + P + S + I + V);

    let mut out = [0u8; OUT];
    let mut n = 0usize;

    let prefix = b"iface\t";
    let mut i = 0usize;
    while i < prefix.len() {
        out[n] = prefix[i];
        n += 1;
        i += 1;
    }

    let mut write_str = |s: &str, out: &mut [u8; OUT], n: &mut usize| {
        let bytes = s.as_bytes();
        let mut j = 0usize;
        while j < bytes.len() {
            out[*n] = bytes[j];
            *n += 1;
            j += 1;
        }
    };

    write_str(package_name, &mut out, &mut n);
    out[n] = b'\t';
    n += 1;
    write_str(self_type_key, &mut out, &mut n);
    out[n] = b'\t';
    n += 1;
    write_str(interface_name, &mut out, &mut n);
    out[n] = b'\t';
    n += 1;
    write_str(interface_version, &mut out, &mut n);
    out[n] = b'\n';
    out
}
```

- [ ] **Step 4: Run the codegen tests and a no-run compile sweep**

Run:

```bash
cargo test -p groundhog-codegen spec_reg --offline
```

Then run:

```bash
HOGD_FFI_LIB_DIR=target/debug/build/groundhog-sys-c7e1afbcc679e113/out/hogd_lib_0.12.1 \
env -u RUSTC_WRAPPER cargo test --offline --manifest-path example-contracts/Cargo.toml \
  --features dev --no-run -p stateless -p counter -p erc20_low_level -p multi_imported_traits
```

Expected: PASS for both commands.

- [ ] **Step 5: Commit raw fragment emission**

```bash
git add gen/src/lib.rs gen/src/spec_reg.rs macros/src/contract.rs src/lib.rs
git commit -m "feat: emit raw interface spec fragments"
```

## Task 4: Add The CLI And Hook The Post-Link Rewrite Into The Build Script

**Files:**
- Create: `groundhog-spec-tool/src/main.rs`
- Modify: `scripts/dev.sh`

- [ ] **Step 1: Add a failing CLI smoke test**

Append this to `groundhog-spec-tool/tests/rewrite.rs`:

```rust
use std::fs;

#[test]
fn cli_rewrite_writes_json_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let wasm_path = dir.path().join("demo.wasm");
    let json_path = dir.path().join("demo.spec.json");

    let mut module = wasm_encoder::Module::new();
    module.section(&wasm_encoder::CustomSection {
        name: "groundhogspecrawv0".into(),
        data: b"iface\tdemo\t\t_unnamed_interface\t0.3.0\n".into(),
    });
    fs::write(&wasm_path, module.finish()).unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_groundhog-spec-tool"))
        .args(["rewrite", "--wasm"])
        .arg(&wasm_path)
        .args(["--json-out"])
        .arg(&json_path)
        .status()
        .unwrap();

    assert!(status.success());
    let json = fs::read_to_string(json_path).unwrap();
    assert!(json.contains("_unnamed_interface"));
}
```

- [ ] **Step 2: Run the CLI rewrite test and verify it fails**

Run:

```bash
cargo test -p groundhog-spec-tool --test rewrite --offline
```

Expected: FAIL because the binary target does not exist and JSON writing is not implemented.

- [ ] **Step 3: Implement the CLI entrypoint and build-script hook**

Create `groundhog-spec-tool/src/main.rs`:

```rust
use std::{env, fs};

use groundhog_spec_tool::{rewrite::rewrite_wasm, raw::FINAL_FORMAT_TAG};

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("rewrite") => {
            let wasm_path = args.next().expect("expected --wasm");
            let json_path = args.next().expect("expected --json-out");
            let bytes = fs::read(&wasm_path).expect("failed to read wasm");
            let rewritten = rewrite_wasm(&bytes).expect("failed to rewrite wasm");
            fs::write(&wasm_path, rewritten).expect("failed to write wasm");
            fs::write(&json_path, format!("{{\"format\":\"{FINAL_FORMAT_TAG}\"}}\n"))
                .expect("failed to write json");
        }
        _ => panic!("usage: groundhog-spec-tool rewrite --wasm <path> --json-out <path>"),
    }
}
```

Modify `scripts/dev.sh`:

```bash
finalize_contract_spec() {
    local package="$1"; shift
    local wasm_name="${package//-/_}"
    local optimized="${OPTIMIZED}/${wasm_name}.wasm"
    local json="${OPTIMIZED}/${wasm_name}.spec.json"

    cargo run --quiet -p groundhog-spec-tool -- \
        rewrite \
        --wasm "${optimized}" \
        --json-out "${json}"
}
```

and call it at the end of `build_contract()`:

```bash
build_wasm "$workspace_dir" "$package" --release &&
make_optimized "$package" &&
finalize_contract_spec "$package"
```

- [ ] **Step 4: Run tool tests and a real build smoke test**

Run:

```bash
cargo test -p groundhog-spec-tool --test rewrite --offline
```

Then run:

```bash
bash -lc 'source scripts/dev.sh && eg_build erc20_low_level'
```

Expected:
- rewrite tests PASS
- build succeeds
- `target/optimized/erc20_low_level.wasm` exists
- `target/optimized/erc20_low_level.spec.json` exists

- [ ] **Step 5: Commit CLI and build integration**

```bash
git add groundhog-spec-tool/src/main.rs scripts/dev.sh groundhog-spec-tool/tests/rewrite.rs
git commit -m "feat: rewrite interface specs into final wasm section"
```

## Task 5: Add End-To-End Example Coverage For Bare Impls, Imported Traits, Named Free Functions, And Unnamed Free Functions

**Files:**
- Create: `example-contracts/free_fn_unnamed/Cargo.toml`
- Create: `example-contracts/free_fn_unnamed/src/lib.rs`
- Create: `example-contracts/free_fn_unnamed/src/implementation.rs`
- Modify: `example-contracts/Cargo.toml`
- Modify: `groundhog-spec-tool/tests/rewrite.rs`

- [ ] **Step 1: Add failing end-to-end artifact assertions**

Append an end-to-end test to `groundhog-spec-tool/tests/rewrite.rs`:

```rust
#[test]
fn built_contracts_produce_expected_final_interface_lists() {
    let fixtures = [
        ("counter", &["Counter\t0.3.0"][..]),
        ("erc20_low_level", &["ERC20\t0.1.0"][..]),
        (
            "multi_imported_traits",
            &[
                "ExplicitNamedInterface\t0.3.0",
                "VersionedCallee\t1.2.3",
            ][..],
        ),
        ("free_fn_unnamed", &["_unnamed_interface\t0.3.0"][..]),
    ];

    for (package, expected_lines) in fixtures {
        let status = std::process::Command::new("bash")
            .arg("-lc")
            .arg(format!("source scripts/dev.sh && eg_build {package}"))
            .status()
            .unwrap();
        assert!(status.success(), "build failed for {package}");

        let wasm_name = package.replace('-', "_");
        let spec_path = format!("target/optimized/{wasm_name}.spec.json");
        let json = std::fs::read_to_string(spec_path).unwrap();
        for line in expected_lines {
            assert!(json.contains(line), "missing `{line}` for {package}");
        }
    }
}
```

- [ ] **Step 2: Run the end-to-end test and verify it fails**

Run:

```bash
cargo test -p groundhog-spec-tool --test rewrite built_contracts_produce_expected_final_interface_lists --offline
```

Expected: FAIL because `free_fn_unnamed` does not exist yet and the JSON output still only contains the temporary format stub from Task 4.

- [ ] **Step 3: Add the unnamed free-function fixture and real JSON output**

Create `example-contracts/free_fn_unnamed/Cargo.toml`:

```toml
[package]
name = "free_fn_unnamed"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
publish.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
sdk.workspace = true
```

Create `example-contracts/free_fn_unnamed/src/lib.rs`:

```rust
#![no_std]

mod implementation;
pub use implementation::*;
```

Create `example-contracts/free_fn_unnamed/src/implementation.rs`:

```rust
use sdk::*;

#[contract]
pub fn ping() -> u32 {
    7
}

#[contract]
pub fn pong() -> u32 {
    8
}
```

Update `example-contracts/Cargo.toml` members:

```toml
members = [
    # ...
    "free_fn_unnamed",
]
```

Then write the real JSON payload from the decoded final section in `groundhog-spec-tool/src/main.rs`:

```rust
let interfaces = groundhog_spec_tool::rewrite::extract_interfaces(&rewritten)
    .expect("failed to decode final section");
let json = serde_json::to_string_pretty(&interfaces).expect("failed to encode json");
fs::write(&json_path, json).expect("failed to write json");
```

- [ ] **Step 4: Run the end-to-end artifact test and targeted build sweeps**

Run:

```bash
cargo test -p groundhog-spec-tool --test rewrite built_contracts_produce_expected_final_interface_lists --offline
```

Then run:

```bash
HOGD_FFI_LIB_DIR=target/debug/build/groundhog-sys-c7e1afbcc679e113/out/hogd_lib_0.12.1 \
env -u RUSTC_WRAPPER cargo test --offline --manifest-path example-contracts/Cargo.toml \
  --features dev --no-run -p counter -p erc20_low_level -p multi_imported_traits -p free_fn_unnamed
```

Expected: PASS for both commands.

- [ ] **Step 5: Commit end-to-end coverage**

```bash
git add example-contracts/Cargo.toml example-contracts/free_fn_unnamed groundhog-spec-tool/tests/rewrite.rs groundhog-spec-tool/src/main.rs
git commit -m "test: verify interface spec rewrite across contract styles"
```

## Task 6: Final Verification And Polish

**Files:**
- Modify: `docs/superpowers/plans/2026-04-04-interface-spec-registration.md`
- Modify: any files touched above only if verification exposes concrete issues

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --all
cargo fmt --all --manifest-path example-contracts/Cargo.toml
```

Expected: formatting succeeds with no errors.

- [ ] **Step 2: Run the focused verification matrix**

Run:

```bash
cargo test -p groundhog-spec-tool --offline
```

Run:

```bash
HOGD_FFI_LIB_DIR=target/debug/build/groundhog-sys-c7e1afbcc679e113/out/hogd_lib_0.12.1 \
env -u RUSTC_WRAPPER cargo test --offline --manifest-path example-contracts/Cargo.toml \
  --features dev --no-run -p stateless -p counter -p erc20_low_level -p multi_imported_traits -p free_fn_unnamed
```

Run:

```bash
bash -lc 'source scripts/dev.sh && eg_build counter && eg_build erc20_low_level && eg_build multi_imported_traits && eg_build free_fn_unnamed'
```

Expected:
- tool crate tests PASS
- example compile sweep PASS
- each optimized wasm has one final `groundhogspecv0` section plus a `.spec.json` sidecar

- [ ] **Step 3: Commit the final polish if any file changed**

```bash
git add groundhog-spec-tool gen macros src scripts example-contracts
git commit -m "chore: finalize interface spec registration"
```

- [ ] **Step 4: Push the branch**

```bash
git push -u origin geoff/interface-registration
```

## Self-Review

- Spec coverage:
  - Raw-at-link emission is covered in Task 3.
  - Post-link normalization and final-section rewrite are covered in Tasks 2 and 4.
  - Free-function named and unnamed behavior is covered in Task 5.
  - Imported trait metadata preservation is covered by `multi_imported_traits` in Task 5.
  - Build-time JSON convenience artifact is covered in Tasks 4 and 5.
- Placeholder scan:
  - No unfinished markers or cross-references that omit concrete instructions remain.
  - Every implementation task names exact files and commands.
- Type consistency:
  - Raw section: `groundhogspecrawv0`
  - Final section: `groundhogspecv0`
  - Final format tag: `gh_iface_v1`
  - Unnamed interface sentinel: `_unnamed_interface`
