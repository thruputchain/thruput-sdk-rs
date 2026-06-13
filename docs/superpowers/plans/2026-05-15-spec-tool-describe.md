# `groundhog-spec-tool describe` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `describe <TypeName>` subcommand to `groundhog-spec-tool` that emits the `ContractSpec` for SDK-shared types (v1: `LibraryExn`) as JSON or XDR, for consumption by downstream bindings (notably the TS SDK).

**Architecture:** Reuse the existing `ContractSpec` pipeline. SDK exception types gain `#[derive(ContractTypeSpec)]` so their per-type `SpecFragmentView` constants become available at compile time. `groundhog-spec-tool` adds a new `canonicalize_type_defs` entry point (parallel to `canonicalize_spec`) that handles type-only fragments without requiring interface fragments, then renders via the existing `render_spec_json` / `WriteXDR` paths. External types from `groundhog-xdr` (`SyscallNo`, `SysErr`) get manual `ContractTypeRefSpec` impls in `groundhog-contract-spec` rather than upstream macros.

**Tech Stack:** Rust 2024 edition, `groundhog-spec-tool` (workspace crate), `sdk` crate (workspace root), `groundhog-contract-spec`, the existing `ContractTypeSpec` / `ContractTypeRefSpec` traits and macros.

**Known prerequisites baked into the task list:**
- `ContractTypeName` has no derive macro (only `#[contracttype]` and `#[contract_exception]` generate it). The SDK exception types use neither, so each one needs a manual `impl ContractTypeName` block — included in the relevant tasks.
- The `ContractTypeSpec` derive emits `const __groundhog_spec_fragment_<Ident>: SpecFragmentView<'static> = ...` (see `macros/src/derive_contract_type_spec.rs:117`). The const is currently private; spec-tool needs to reference it from another crate. Fix is a one-line change to the macro: emit `pub const` instead of `const`. Done in its own task before the derives are added.

**Branch:** `geoff/spec-tool-describe` (already created).

**Per user preference: do NOT `git add` files under `docs/superpowers/`. The spec and plan documents stay as local working artifacts and are excluded from all commits in this plan.**

---

## File Structure

- `groundhog-spec-tool/src/lib.rs` — add `canonicalize_type_defs` entry point (parallel to `canonicalize_spec`).
- `groundhog-spec-tool/src/main.rs` — add `describe` subcommand, type registry, output handling.
- `groundhog-spec-tool/src/describe.rs` — new module: type registry, fragment-gathering helpers, output formatting.
- `groundhog-spec-tool/Cargo.toml` — add `sdk` dependency with `spec` + `storage` features.
- `groundhog-spec-tool/tests/describe.rs` — new test file: completeness test + snapshot test.
- `groundhog-contract-spec/src/groundhog_xdr_types.rs` — add manual `ContractTypeRefSpec` impls for `SyscallNo` and `SysErr`.
- `src/exn/library.rs` — explicit discriminants + `ContractTypeSpec` derive on `LibraryExn` and `SyscallExn`.
- `src/exn/wrapper.rs` — explicit discriminants + `ContractTypeSpec` derive on `CallingError`, `HandlingError`, `AuthTokenError`.
- `src/exn/fault.rs` — `ContractTypeSpec` derive on `without_strings::FaultString`.
- `src/storage/error.rs` — explicit discriminants + `ContractTypeSpec` derive on `StorageError`.

---

## Task 1: Add `canonicalize_type_defs` entry point

**Files:**
- Modify: `groundhog-spec-tool/src/lib.rs`
- Test: `groundhog-spec-tool/src/lib.rs` (inline `#[cfg(test)]` module)

**Context:** `canonicalize_spec` errors with `NoSpecFragments` when `interfaces` is empty. The describe path has type defs but no interfaces by design. Add a sibling entry point that accepts a roots list and reuses `resolve_reachable_type_defs`.

- [ ] **Step 1: Write failing test**

Add this test at the bottom of `groundhog-spec-tool/src/lib.rs`:

```rust
#[cfg(test)]
mod canonicalize_type_defs_tests {
    use super::*;
    use groundhog_contract_spec::{
        ContractEnumDef, ContractEnumVariant, ContractTypeDef,
    };

    fn enum_def(name: &str) -> ContractTypeDef {
        ContractTypeDef::TypeDefEnum {
            enum_def: ContractEnumDef {
                name: name.as_bytes().try_into().unwrap(),
                variants: vec![ContractEnumVariant {
                    name: b"A".as_slice().try_into().unwrap(),
                    value: 0,
                }]
                .try_into()
                .unwrap(),
            },
        }
    }

    #[test]
    fn canonicalize_type_defs_resolves_root() {
        let fragments = vec![SpecFragment::SpecTypeDef {
            type_def: enum_def("MyEnum"),
        }];
        let roots = vec![b"MyEnum".as_slice().try_into().unwrap()];

        let spec = canonicalize_type_defs(&fragments, &roots)
            .expect("canonicalize_type_defs should succeed");

        assert!(spec.interfaces.is_empty());
        assert_eq!(spec.type_defs.len(), 1);
    }

    #[test]
    fn canonicalize_type_defs_errors_on_missing_root() {
        let fragments: Vec<SpecFragment> = vec![];
        let roots = vec![b"MissingType".as_slice().try_into().unwrap()];

        let err = canonicalize_type_defs(&fragments, &roots)
            .expect_err("missing root should error");
        assert!(matches!(err, SpecToolError::MissingTypeDef(_)));
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p groundhog-spec-tool canonicalize_type_defs_`
Expected: FAIL with "cannot find function `canonicalize_type_defs`".

- [ ] **Step 3: Add `canonicalize_type_defs` to `groundhog-spec-tool/src/lib.rs`**

Insert this function below the existing `canonicalize_spec` function (around line 230):

```rust
/// Assembles a [`ContractSpec`] whose `interfaces` list is empty and whose
/// `type_defs` are the transitive closure of `roots` over the supplied
/// fragments. Use this when you want to emit specs for named types without
/// any associated interface (e.g. the `describe` subcommand).
pub fn canonicalize_type_defs(
    raw: &[SpecFragment],
    roots: &[ContractSpecString],
) -> Result<ContractSpec, SpecToolError> {
    let mut type_defs =
        BTreeMap::<ContractSpecString, Vec<ContractTypeDef>>::new();
    let mut exported_roots =
        std::collections::BTreeSet::<ContractSpecString>::new();

    for fragment in raw {
        match fragment {
            SpecFragment::SpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            SpecFragment::ExportedSpecTypeDef { type_def } => {
                let symbol = type_def_symbol(type_def).clone();
                exported_roots.insert(symbol.clone());
                let defs = type_defs.entry(symbol).or_default();
                if !defs.iter().any(|existing| existing == type_def) {
                    defs.push(type_def.clone());
                }
            }
            _ => {}
        }
    }

    for symbol in roots {
        exported_roots.insert(symbol.clone());
    }

    for (symbol, defs) in &type_defs {
        if defs.len() > 1 {
            return Err(SpecToolError::ConflictingTypeDef(
                spec_str(symbol).to_owned(),
            ));
        }
    }

    let interfaces: Vec<ContractSpecInterface> = Vec::new();
    let resolved =
        resolve_reachable_type_defs(&interfaces, &type_defs, &exported_roots)?;
    Ok(ContractSpec {
        interfaces: interfaces.try_into().unwrap(),
        type_defs: resolved.try_into().unwrap(),
    })
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p groundhog-spec-tool canonicalize_type_defs_`
Expected: 2 tests pass.

- [ ] **Step 5: Run the full spec-tool test suite to confirm no regressions**

Run: `cargo test -p groundhog-spec-tool`
Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add groundhog-spec-tool/src/lib.rs
git commit -m "feat(spec-tool): add canonicalize_type_defs entry point"
```

---

## Task 2: Manual `ContractTypeRefSpec` impls for `SyscallNo` and `SysErr`

**Files:**
- Modify: `groundhog-contract-spec/src/groundhog_xdr_types.rs`

**Context:** `SyscallNo` (defined as `u64` in `groundhog-xdr`) and `SysErr` (defined as `i32` newtype `SysErr(pub i32)` in `groundhog-xdr::syscalls`) are used as fields in `SyscallExn`. Deriving `ContractTypeSpec` on `SyscallExn` requires both to implement `ContractTypeRefSpec`. We add the impls in `groundhog-contract-spec` rather than upstream so `groundhog-xdr` stays free of dependencies on the macros/spec crate.

- [ ] **Step 1: Write failing test**

Add a test file `groundhog-contract-spec/tests/groundhog_xdr_refspec.rs`:

```rust
use groundhog_contract_spec::{
    ContractTypeRefSpec, view::ContractTypeView,
};
use groundhog_xdr::{SyscallNo, syscalls::SysErr};

#[test]
fn syscallno_refspec_is_u64() {
    assert!(matches!(
        <SyscallNo as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::U64
    ));
}

#[test]
fn syserr_refspec_is_i32() {
    assert!(matches!(
        <SysErr as ContractTypeRefSpec>::INFO.ty,
        ContractTypeView::I32
    ));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p groundhog-contract-spec --test groundhog_xdr_refspec`
Expected: FAIL with "the trait `ContractTypeRefSpec` is not implemented for `SyscallNo`" (and same for `SysErr`).

- [ ] **Step 3: Add the impls**

Append to `groundhog-contract-spec/src/groundhog_xdr_types.rs`:

```rust
use groundhog_xdr::{SyscallNo, syscalls::SysErr};

impl ContractTypeRefSpec for SyscallNo {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::U64,
    };
}

impl ContractTypeRefSpec for SysErr {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::I32,
    };
}
```

Update the existing `use` statement at the top of that file (line 1-7) to also import `SyscallNo`:

```rust
use groundhog_xdr::{
    SyscallNo,
    contract::ContractEntrypointID,
    syscalls::SysErr,
    types::{
        BlockHeight, ContractAddress, Ed25519PublicKey, Ed25519Signature,
        SHA256Hash,
    },
};
```

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p groundhog-contract-spec --test groundhog_xdr_refspec`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add groundhog-contract-spec/src/groundhog_xdr_types.rs groundhog-contract-spec/tests/groundhog_xdr_refspec.rs
git commit -m "feat(contract-spec): add ContractTypeRefSpec impls for SyscallNo and SysErr"
```

---

## Task 3: Explicit discriminants on `LibraryExn`

**Files:**
- Modify: `src/exn/library.rs`

**Context:** The `ContractTypeSpec` derive enforces explicit discriminants on every enum variant. Adding `0..=N` discriminants matches what the compiler already assigns by default for `#[repr(i32)]` enums, so wire format is preserved.

- [ ] **Step 1: Update `LibraryExn` definition to use explicit discriminants**

In `src/exn/library.rs`, replace the enum body (lines 22-53) with:

```rust
pub enum LibraryExn {
    #[cfg_attr(feature = "debugging", error(transparent))]
    Syscall(SyscallExn) = 0,

    #[cfg_attr(feature = "debugging", error("Handling a contract call: {0}"))]
    Handling(HandlingError) = 1,

    #[cfg_attr(feature = "debugging", error("Making a contract call: {0}"))]
    Calling(CallingError) = 2,

    #[cfg_attr(feature = "debugging", error("Called contract reverted: {0}"))]
    Called(#[cfg_attr(feature = "debugging", source)] FaultString) = 3,

    #[cfg_attr(feature = "debugging", error("Encoding auth token: {0}"))]
    AuthToken(AuthTokenError) = 4,

    #[cfg_attr(
        feature = "debugging",
        error("bytestring_get_sized: expected {expected}, found {found}")
    )]
    BytestringWrongSize { expected: u32, found: u32 } = 5,

    #[cfg_attr(feature = "debugging", error("non-negative int went negative"))]
    NonNegativeIntUnderflow = 6,

    #[cfg_attr(feature = "debugging", error("non-negative int overflow"))]
    NonNegativeIntOverflow = 7,

    #[cfg(feature = "storage")]
    #[cfg_attr(feature = "debugging", error("Storage error: {0}"))]
    Storage(StorageError) = 8,
}
```

- [ ] **Step 2: Run existing selector tests**

Run: `cargo test -p sdk --lib exn::library`
Expected: both `library_exn_selector_matches_sha256` and `library_exn_selector_nonzero` still pass.

- [ ] **Step 3: Run the wider SDK test suite to confirm nothing breaks**

Run: `cargo test -p sdk --lib`
Expected: all tests still pass.

- [ ] **Step 4: Commit**

```bash
git add src/exn/library.rs
git commit -m "refactor(exn): add explicit discriminants to LibraryExn variants"
```

---

## Task 4: Explicit discriminants on `CallingError`, `HandlingError`, `AuthTokenError`

**Files:**
- Modify: `src/exn/wrapper.rs`

**Context:** Same rationale as Task 3 — explicit discriminants matching what the compiler assigns implicitly.

- [ ] **Step 1: Update `CallingError`**

In `src/exn/wrapper.rs`, replace the `CallingError` body (lines 14-35):

```rust
pub enum CallingError {
    #[cfg_attr(
        feature = "debugging",
        error("Invalid call: contract not found (or bad auth tokens)")
    )]
    Invalid = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Decoding a successful result: {0}")
    )]
    DecodingSuccess(FaultString) = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Decoding an exception result: {0}")
    )]
    DecodingException(FaultString) = 2,

    #[cfg_attr(feature = "debugging", error("Encoding arguments: {0}"))]
    EncodingArgs(FaultString) = 3,
}
```

- [ ] **Step 2: Update `HandlingError`**

Replace the `HandlingError` body (lines 41-56):

```rust
pub enum HandlingError {
    #[cfg_attr(feature = "debugging", error("Decoding argument data: {0}"))]
    DecodingArgs(FaultString) = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Encoding successful result: {0}")
    )]
    EncodingSuccess(FaultString) = 1,

    #[cfg_attr(feature = "debugging", error("Encoding exception result: {0}"))]
    EncodingException(FaultString) = 2,

    #[cfg_attr(feature = "debugging", error("Encoding event: {0}"))]
    EncodingEvent(FaultString) = 3,
}
```

- [ ] **Step 3: Update `AuthTokenError`**

Replace the `AuthTokenError` body (lines 62-76):

```rust
pub enum AuthTokenError {
    #[cfg_attr(feature = "debugging", error("Encoding auth token: {0}"))]
    MintEncodeFailure(FaultString) = 0,
    #[cfg_attr(
        feature = "debugging",
        error("Decoding auth token payload: {0}")
    )]
    ConsumeDecodeFailure(FaultString) = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Receiving auth token handles: {0}")
    )]
    ReceiveAuthHandlesFailure(FaultString) = 2,
}
```

- [ ] **Step 4: Run SDK test suite**

Run: `cargo test -p sdk --lib`
Expected: all tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/exn/wrapper.rs
git commit -m "refactor(exn): add explicit discriminants to wrapper error enums"
```

---

## Task 5: Explicit discriminants on `StorageError`

**Files:**
- Modify: `src/storage/error.rs`

- [ ] **Step 1: Update the enum**

In `src/storage/error.rs`, replace the `StorageError` body (lines 14-29):

```rust
pub enum StorageError {
    #[cfg_attr(feature = "debugging", error(transparent))]
    Sys { exn: SyscallExn } = 0,

    #[cfg_attr(
        feature = "debugging",
        error("Storage error due to serialization")
    )]
    Serialization = 1,

    #[cfg_attr(
        feature = "debugging",
        error("Vector index out of bounds: len={len}, access={access}")
    )]
    IndexOutOfBounds { len: u64, access: u64 } = 2,
}
```

- [ ] **Step 2: Run storage tests**

Run: `cargo test -p sdk --test storage --features storage`
Expected: all tests still pass.

- [ ] **Step 3: Commit**

```bash
git add src/storage/error.rs
git commit -m "refactor(storage): add explicit discriminants to StorageError"
```

---

## Task 6: Make `ContractTypeSpec` fragment consts public

**Files:**
- Modify: `macros/src/derive_contract_type_spec.rs`

**Context:** The derive emits `const __groundhog_spec_fragment_<Ident>: SpecFragmentView<'static> = ...` privately. Spec-tool needs to reference these from another crate, so they need to be `pub`. The `#[doc(hidden)]` attribute keeps them out of generated docs.

- [ ] **Step 1: Modify the macro**

In `macros/src/derive_contract_type_spec.rs`, line 117, replace:

```rust
        #[doc(hidden)]
        const #fragment_ident: ::sdk::__contract_spec::SpecFragmentView<'static> =
            #fragment_variant;
```

with:

```rust
        #[doc(hidden)]
        pub const #fragment_ident: ::sdk::__contract_spec::SpecFragmentView<'static> =
            #fragment_variant;
```

- [ ] **Step 2: Build the macros crate**

Run: `cargo build -p groundhog-macros`
Expected: builds successfully.

- [ ] **Step 3: Run macros tests**

Run: `cargo test -p groundhog-macros`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add macros/src/derive_contract_type_spec.rs
git commit -m "refactor(macros): emit ContractTypeSpec fragment consts as pub"
```

---

## Task 7: `ContractTypeSpec` derive and `ContractTypeName` impls on `FaultString` and `SyscallExn`

**Files:**
- Modify: `src/exn/fault.rs`
- Modify: `src/exn/library.rs`

**Context:** `FaultString` has two cfg-gated definitions. We add `ContractTypeSpec` to the `without_strings` (unit struct) version only — that's the wire form used by compiled contracts (per user decision). The `with_strings` version is for native testing/debugging and is not spec-emitted. Spec-tool builds the SDK without the `debugging` feature, so only the unit-struct version is in scope when fragments are gathered.

`SyscallExn` is a struct containing `SyscallNo` and `SysErr` fields, both of which now have `ContractTypeRefSpec` impls (Task 2).

Each type also needs a manual `impl ContractTypeName` since there's no derive for it.

- [ ] **Step 1: Modify `without_strings::FaultString` in `src/exn/fault.rs`**

In `src/exn/fault.rs`, inside `mod without_strings` (line 88-137), modify the struct definition (line 93) to add the derive:

```rust
    #[derive(Debug, Clone, PartialEq, ::sdk::ContractTypeSpec)]
    pub struct FaultString;
```

Note: use `::sdk::ContractTypeSpec` (the macro is re-exported at the crate root, see `src/lib.rs:124`). Since this is `src/`, the crate root is `sdk`; `crate::ContractTypeSpec` also works.

Add a manual `ContractTypeName` impl inside `mod without_strings`, after the existing `impl FaultString { ... }` block (around line 136):

```rust
    impl ::sdk::__contract_spec::ContractTypeName for FaultString {
        const NAME: &'static str = "FaultString";
    }
```

The `with_strings::FaultString` is untouched.

- [ ] **Step 2: Modify `SyscallExn` in `src/exn/library.rs`**

At line 147, add `ContractTypeSpec` to the derive list:

```rust
#[derive(Debug, Clone, Copy, ReadXDR, WriteXDR, PartialEq, ContractTypeSpec)]
#[cfg_attr(feature = "debugging", derive(Error))]
#[cfg_attr(feature = "spec", derive(XDR))]
pub struct SyscallExn {
    callno: SyscallNo,
    errno: SysErr,
}
```

Add `ContractTypeSpec` to the existing import on line 7 (already imports `xdr::*` which brings ReadXDR/WriteXDR in). Add this near the top of the file:

```rust
use crate::ContractTypeSpec;
```

After the `impl SyscallExn { ... }` block (line 156-171), add:

```rust
impl crate::__contract_spec::ContractTypeName for SyscallExn {
    const NAME: &'static str = "SyscallExn";
}
```

- [ ] **Step 3: Build with spec feature**

Run: `cargo build -p sdk --features spec`
Expected: builds successfully.

- [ ] **Step 4: Run lib tests**

Run: `cargo test -p sdk --lib`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/exn/fault.rs src/exn/library.rs
git commit -m "feat(exn): derive ContractTypeSpec on FaultString and SyscallExn"
```

---

## Task 8: `ContractTypeSpec` derive and `ContractTypeName` impls on wrapper enums and `StorageError`

**Files:**
- Modify: `src/exn/wrapper.rs`
- Modify: `src/storage/error.rs`

- [ ] **Step 1: Update derives in `src/exn/wrapper.rs`**

For each of `CallingError` (line 11), `HandlingError` (line 38), `AuthTokenError` (line 59), modify the derive line to include `ContractTypeSpec`:

```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec)]
```

Add to the top of the file:

```rust
use crate::ContractTypeSpec;
```

At the bottom of the file, add manual `ContractTypeName` impls:

```rust
impl crate::__contract_spec::ContractTypeName for CallingError {
    const NAME: &'static str = "CallingError";
}

impl crate::__contract_spec::ContractTypeName for HandlingError {
    const NAME: &'static str = "HandlingError";
}

impl crate::__contract_spec::ContractTypeName for AuthTokenError {
    const NAME: &'static str = "AuthTokenError";
}
```

- [ ] **Step 2: Update derive in `src/storage/error.rs`**

At line 10, modify:

```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec)]
```

Add to the top:

```rust
use crate::ContractTypeSpec;
```

At the bottom of the file, add:

```rust
impl crate::__contract_spec::ContractTypeName for StorageError {
    const NAME: &'static str = "StorageError";
}
```

- [ ] **Step 3: Build with spec + storage features**

Run: `cargo build -p sdk --features "spec storage"`
Expected: builds successfully.

- [ ] **Step 4: Run SDK lib tests**

Run: `cargo test -p sdk --lib --features storage`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/exn/wrapper.rs src/storage/error.rs
git commit -m "feat(exn): derive ContractTypeSpec on wrapper enums and StorageError"
```

---

## Task 9: `ContractTypeSpec` derive and `ContractTypeName` impl on `LibraryExn`

**Files:**
- Modify: `src/exn/library.rs`

- [ ] **Step 1: Add derive**

At `src/exn/library.rs` line 17, modify the derive:

```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, ContractTypeSpec)]
```

(The `use crate::ContractTypeSpec;` import was already added in Task 7 Step 2.)

- [ ] **Step 2: Add `ContractTypeName` impl**

After the `impl LibraryExn { ... }` block (around line 88), add:

```rust
impl crate::__contract_spec::ContractTypeName for LibraryExn {
    const NAME: &'static str = "LibraryExn";
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p sdk --features "spec storage"`
Expected: builds successfully. The transitive closure is satisfied: `LibraryExn` → `SyscallExn`, `HandlingError`, `CallingError`, `FaultString`, `AuthTokenError`, `StorageError`; primitives `u32`/`i32` are already impl'd in `groundhog-contract-spec/src/lib.rs`; `SyscallNo`/`SysErr` from Task 2.

- [ ] **Step 4: Run lib tests**

Run: `cargo test -p sdk --lib --features storage`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/exn/library.rs
git commit -m "feat(exn): derive ContractTypeSpec on LibraryExn"
```

---

## Task 10: Add `describe` subcommand to `groundhog-spec-tool`

**Files:**
- Modify: `groundhog-spec-tool/Cargo.toml`
- Create: `groundhog-spec-tool/src/describe.rs`
- Modify: `groundhog-spec-tool/src/lib.rs`
- Modify: `groundhog-spec-tool/src/main.rs`

**Context:** Pull in the `sdk` dependency (with `spec` + `storage` features, no `debugging`) so the `ContractTypeSpec` fragments for `LibraryExn` and its closure are reachable. Build the type registry, CLI parsing, and output dispatch.

- [ ] **Step 1: Add `sdk` dependency to `groundhog-spec-tool/Cargo.toml`**

In `groundhog-spec-tool/Cargo.toml`, append to `[dependencies]`:

```toml
sdk = { workspace = true, default-features = false, features = ["spec", "storage"] }
```

- [ ] **Step 2: Create `groundhog-spec-tool/src/describe.rs`**

```rust
use crate::{
    canonicalize_type_defs, render_spec_json, SpecToolError,
};
use groundhog_contract_spec::{
    ContractSpec, ContractSpecString, SpecFragment, SpecFragmentView,
};
use std::io::{self, Write};
use xdrs::txs;

/// A named SDK-shared type that `describe` can emit.
pub struct DescribableType {
    pub name: &'static str,
    pub fragments: &'static [SpecFragmentView<'static>],
    pub root: &'static str,
}

const LIBRARY_EXN_FRAGMENTS: &[SpecFragmentView<'static>] = &[
    // Order doesn't matter — canonicalize handles dedup.
    sdk::exn::library::__groundhog_spec_fragment_LibraryExn,
    sdk::exn::library::__groundhog_spec_fragment_SyscallExn,
    sdk::exn::wrapper::__groundhog_spec_fragment_CallingError,
    sdk::exn::wrapper::__groundhog_spec_fragment_HandlingError,
    sdk::exn::wrapper::__groundhog_spec_fragment_AuthTokenError,
    sdk::exn::fault::__groundhog_spec_fragment_FaultString,
    sdk::storage::error::__groundhog_spec_fragment_StorageError,
];

pub const REGISTRY: &[DescribableType] = &[DescribableType {
    name: "LibraryExn",
    fragments: LIBRARY_EXN_FRAGMENTS,
    root: "LibraryExn",
}];

pub fn lookup(name: &str) -> Option<&'static DescribableType> {
    REGISTRY.iter().find(|t| t.name == name)
}

pub fn build_spec(ty: &DescribableType) -> Result<ContractSpec, SpecToolError> {
    let fragments: Vec<SpecFragment> = ty
        .fragments
        .iter()
        .map(|view| SpecFragment::from(*view))
        .collect();
    let root: ContractSpecString = ty.root.as_bytes().try_into().unwrap();
    canonicalize_type_defs(&fragments, &[root])
}

pub enum OutputFormat {
    Json,
    Xdr,
}

pub fn write_output<W: Write>(
    spec: &ContractSpec,
    format: OutputFormat,
    sink: &mut W,
) -> io::Result<()> {
    match format {
        OutputFormat::Json => {
            sink.write_all(render_spec_json(spec).as_bytes())?;
            sink.write_all(b"\n")?;
        }
        OutputFormat::Xdr => {
            let bytes = txs::encode(spec)
                .map_err(|e| io::Error::other(format!("xdr encode: {e}")))?;
            sink.write_all(&bytes)?;
        }
    }
    Ok(())
}
```

NOTE: the `__groundhog_spec_fragment_<Ident>` consts are now `pub` (Task 6) and live in the same module as the type they describe. The paths above assume the types are at `sdk::exn::library::*`, `sdk::exn::wrapper::*`, `sdk::exn::fault::*`, and `sdk::storage::error::*`. Verify by `grep`ping the SDK lib for the type's pub module path if any path resolves to a private item.

- [ ] **Step 3: Wire the module into `groundhog-spec-tool/src/lib.rs`**

Add `pub mod describe;` near the top of `groundhog-spec-tool/src/lib.rs`.

- [ ] **Step 4: Add the `describe` subcommand to `groundhog-spec-tool/src/main.rs`**

Modify the `run` function to dispatch on `describe`, and add a `run_describe` helper. Replace the existing `match args.next().as_deref()` block (around line 27):

```rust
fn run() -> Result<(), CliError> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("rewrite") => run_rewrite(args),
        Some("describe") => run_describe(args),
        Some(_) => Err(CliError::Usage("expected subcommand 'rewrite' or 'describe'")),
        None => Err(CliError::Usage("expected subcommand 'rewrite' or 'describe'")),
    }
}
```

Add the `run_describe` function below `run_rewrite`:

```rust
fn run_describe(args: impl Iterator<Item = String>) -> Result<(), CliError> {
    use groundhog_spec_tool::describe::{
        build_spec, lookup, write_output, OutputFormat, REGISTRY,
    };

    let mut list = false;
    let mut type_name: Option<String> = None;
    let mut format: Option<OutputFormat> = None;
    let mut output_path: Option<PathBuf> = None;

    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => list = true,
            "--json" => {
                if format.is_some() {
                    return Err(CliError::Usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Json);
            }
            "--xdr" => {
                if format.is_some() {
                    return Err(CliError::Usage(
                        "--json and --xdr are mutually exclusive",
                    ));
                }
                format = Some(OutputFormat::Xdr);
            }
            "-o" => {
                let value = args
                    .next()
                    .ok_or(CliError::Usage("missing value for -o"))?;
                output_path = Some(PathBuf::from(value));
            }
            other if !other.starts_with('-') => {
                if type_name.is_some() {
                    return Err(CliError::Usage(
                        "describe takes a single type name",
                    ));
                }
                type_name = Some(other.to_string());
            }
            _ => return Err(CliError::Usage("unexpected argument")),
        }
    }

    if list {
        if type_name.is_some() {
            return Err(CliError::Usage(
                "--list is mutually exclusive with a type name",
            ));
        }
        for entry in REGISTRY {
            println!("{}", entry.name);
        }
        return Ok(());
    }

    let type_name = type_name
        .ok_or(CliError::Usage("expected type name or --list"))?;
    let ty = lookup(&type_name)
        .ok_or(CliError::Usage("unknown type name"))?;
    let spec = build_spec(ty)?;
    let format = format.unwrap_or(OutputFormat::Json);

    match output_path {
        Some(path) => {
            let mut file = fs::File::create(&path)?;
            write_output(&spec, format, &mut file)?;
        }
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            write_output(&spec, format, &mut handle)?;
        }
    }
    Ok(())
}
```

Add the new `use` statements at the top of `groundhog-spec-tool/src/main.rs`:

```rust
use std::io;
```

(`fs`, `PathBuf` are already imported.)

- [ ] **Step 5: Build the binary**

Run: `cargo build -p groundhog-spec-tool`
Expected: builds successfully.

- [ ] **Step 6: Smoke-test the CLI**

Run: `cargo run -p groundhog-spec-tool -- describe --list`
Expected stdout: `LibraryExn`.

Run: `cargo run -p groundhog-spec-tool -- describe LibraryExn`
Expected: a single line of JSON (the canonical render). It should parse as JSON and contain `"format"`, `"interfaces": []`, and `"type_defs"` with `LibraryExn` plus its closure.

Run: `cargo run -p groundhog-spec-tool -- describe LibraryExn --xdr -o /tmp/library_exn.xdr`
Expected: file `/tmp/library_exn.xdr` is created with binary content.

Run: `cargo run -p groundhog-spec-tool -- describe LibraryExn --json --xdr`
Expected exit code: non-zero with error `--json and --xdr are mutually exclusive`.

- [ ] **Step 7: Commit**

```bash
git add groundhog-spec-tool/Cargo.toml groundhog-spec-tool/src/describe.rs groundhog-spec-tool/src/lib.rs groundhog-spec-tool/src/main.rs
git commit -m "feat(spec-tool): add describe subcommand for SDK-shared types"
```

---

## Task 11: Completeness test — every `Named` reference resolves

**Files:**
- Create: `groundhog-spec-tool/tests/describe.rs`

**Context:** Walk every `ContractType::Named` reference in the output spec and assert each resolves to a `type_def`. Protects against adding a future type to the closure but forgetting to register its fragment.

- [ ] **Step 1: Write the failing test**

Create `groundhog-spec-tool/tests/describe.rs`:

```rust
use groundhog_contract_spec::{
    ContractSpec, ContractType, ContractTypeDef, ContractUnionCaseType,
};
use groundhog_spec_tool::describe::{build_spec, REGISTRY};
use std::collections::BTreeSet;

fn walk_type(ty: &ContractType, refs: &mut BTreeSet<String>) {
    match ty {
        ContractType::Option { option_inner } => walk_type(option_inner, refs),
        ContractType::Vector { vector } => walk_type(&vector.element, refs),
        ContractType::Array { array } => walk_type(&array.element, refs),
        ContractType::Tuple { tuple } => {
            for item in tuple.iter() {
                walk_type(item, refs);
            }
        }
        ContractType::Named { named } => {
            refs.insert(std::str::from_utf8(named).unwrap().to_string());
        }
        _ => {}
    }
}

fn collect_named_refs(spec: &ContractSpec) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for type_def in spec.type_defs.iter() {
        match type_def {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                for field in struct_def.fields.iter() {
                    walk_type(&field.ty, &mut refs);
                }
            }
            ContractTypeDef::TypeDefEnum { .. } => {}
            ContractTypeDef::TypeDefUnion { union_def } => {
                for case in union_def.cases.iter() {
                    match &case.case_type {
                        ContractUnionCaseType::UnionCaseTuple { tuple } => {
                            for ty in tuple.iter() {
                                walk_type(ty, &mut refs);
                            }
                        }
                        ContractUnionCaseType::UnionCaseStruct {
                            struct_fields,
                        } => {
                            for field in struct_fields.iter() {
                                walk_type(&field.ty, &mut refs);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    refs
}

fn defined_names(spec: &ContractSpec) -> BTreeSet<String> {
    spec.type_defs
        .iter()
        .map(|td| {
            let name = match td {
                ContractTypeDef::TypeDefStruct { struct_def } => &struct_def.name,
                ContractTypeDef::TypeDefEnum { enum_def } => &enum_def.name,
                ContractTypeDef::TypeDefUnion { union_def } => &union_def.name,
                _ => panic!("unknown ContractTypeDef variant"),
            };
            std::str::from_utf8(name).unwrap().to_string()
        })
        .collect()
}

#[test]
fn every_registered_type_has_complete_closure() {
    for entry in REGISTRY {
        let spec = build_spec(entry)
            .unwrap_or_else(|e| panic!("build_spec failed for {}: {e:?}", entry.name));
        let refs = collect_named_refs(&spec);
        let defined = defined_names(&spec);
        let missing: Vec<_> = refs.difference(&defined).cloned().collect();
        assert!(
            missing.is_empty(),
            "type {}: dangling Named references: {:?}",
            entry.name,
            missing
        );
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p groundhog-spec-tool --test describe every_registered_type_has_complete_closure`
Expected: PASS (the registry should be complete after Task 9).

If it fails listing missing types, add their fragments to `LIBRARY_EXN_FRAGMENTS` in `groundhog-spec-tool/src/describe.rs`, then re-run.

- [ ] **Step 3: Commit**

```bash
git add groundhog-spec-tool/tests/describe.rs
git commit -m "test(spec-tool): assert describe output has no dangling Named refs"
```

---

## Task 12: Snapshot test for `describe LibraryExn` JSON

**Files:**
- Modify: `groundhog-spec-tool/tests/describe.rs`

**Context:** A snapshot test pins the JSON shape so accidental changes to the type's spec are visible in diffs.

- [ ] **Step 1: Generate the snapshot once and add to test**

Run: `cargo run -p groundhog-spec-tool -- describe LibraryExn --json > /tmp/library_exn.snapshot.json`

Open `/tmp/library_exn.snapshot.json`, paste its content into the test below. Use a raw string literal for readability.

Append to `groundhog-spec-tool/tests/describe.rs`:

```rust
#[test]
fn library_exn_json_snapshot() {
    let ty = groundhog_spec_tool::describe::lookup("LibraryExn")
        .expect("LibraryExn registered");
    let spec = build_spec(ty).expect("build_spec");
    let actual = groundhog_spec_tool::render_spec_json(&spec);
    let expected = r#"<PASTE GENERATED JSON HERE>"#;
    assert_eq!(
        actual, expected,
        "JSON snapshot drift — if intentional, regenerate the snapshot"
    );
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p groundhog-spec-tool --test describe library_exn_json_snapshot`
Expected: PASS.

- [ ] **Step 3: Run the full spec-tool test suite as a final sanity check**

Run: `cargo test -p groundhog-spec-tool`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add groundhog-spec-tool/tests/describe.rs
git commit -m "test(spec-tool): snapshot LibraryExn JSON output"
```

---

## Final verification

- [ ] Run the full workspace test suite:

```bash
cargo test --workspace
```

Expected: all tests pass. No new warnings introduced.

- [ ] Confirm no `docs/superpowers/` files were committed:

```bash
git log main..HEAD -- docs/superpowers/
```

Expected: empty output.
