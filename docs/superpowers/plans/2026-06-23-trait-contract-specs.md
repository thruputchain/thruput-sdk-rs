# Trait Contract Specs Implementation Plan (rv32-native)

> **Revision (2026-06-24): Tasks 2 & 4 dropped as redundant.** groundhog-rpc
> already extracts rv32 specs from the ELF (`thruput build --target rv32` →
> `build_rv32_project` → `finalize_rv32_spec` → `load_rv32_contract_spec_from_bytes`,
> using `groundhog_rv32_lower::elf::parse_elf` + `groundhog_spec_tool::{parse_raw_fragments,
> canonicalize_spec}`). So the `object`-based ELF reader / `spec --elf` (Task 2)
> and the sdk_rs e2e test (Task 4) were removed — no unneeded `object` dependency,
> and rv32 integration tests live in groundhog-rpc. The essential sdk_rs work is
> **Task 1** (macro emits trait fragments) + **Task 3** (erc20-if rv32 carrier bin).
> See `docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a canonical contract spec from a `#[contract]` trait (ERC20 first) with no concrete impl, generated and extracted on the **rv32 path** with no dependency on the wasm target.

**Architecture:** (1) `groundhog-macros` emits the `groundhogspecrawv0` fragments for a trait, same as for impls. (2) Each standard-interface crate gains an `entrypoints` feature and a thin `[[bin]]` wrapper so it builds into a spec-carrying rv32 ELF (the established `extern crate <lib>;` pattern used by `itc_erc20`). (3) `groundhog-spec-tool` gains an **ELF reader** that pulls the `groundhogspecrawv0` section out of a riscv32 ELF and feeds the existing `parse_raw_fragments` → `canonicalize_spec` → `spec_to_json` pipeline. This closes the gap where rv32 spec fragments were emitted but never extracted in-repo.

**Tech Stack:** Rust proc-macros (`syn`/`quote`/`darling`), the `groundhog-spec-tool` crate, the `object` crate (ELF reading), `riscv32im-unknown-none-elf`.

## Global Constraints

- **rv32 is the target; do not add a wasm dependency.** Verification and tests run against the riscv32 ELF, not wasm. `cdylib` is the wasm crate-type and is intentionally NOT added — rv32 builds a `[[bin]]`.
- The fragment-emission cfg gate must be **verbatim**: `#[cfg(all(any(target_family = "wasm", target_arch = "riscv32"), feature = "entrypoints"))]`. Reuse `spec_reg::emit_raw_fragment_tokens` / `emit_root_anchor_tokens`, which already encode it. Do not hand-roll the cfg. (The `target_family = "wasm"` arm stays in the shared gate — removing wasm support entirely is a separate future change, out of scope here.)
- `entrypoints` is a **per-crate feature read by the macro via `cfg!(feature = "entrypoints")`**, not an `sdk` feature. Declare it as `entrypoints = []`. Do NOT add `sdk/entrypoints` (it does not exist). The wasm/rv32 runtimes (`groundhog-rv32-runtime`) are unconditional `sdk` deps, so `_start`/panic handler/allocator come in automatically.
- **Do not** make any contract crate's `entrypoints` feature forward to an interface dependency's `entrypoints` (e.g. never add `groundhog-erc20-if/entrypoints` to `erc20`'s `entrypoints`). That would compile the interface's fragments into the contract artifact. Verified today: no crate does this.
- The trait-derived interface fragment must use the same name/version the trait branch already emits into its `__GROUNDHOG_IFACE_*` consts — `effective_interface_name(&contract)` and `contract.defaulted_interface_vers()` yield exactly these.
- Branch: `geoff/trait-contract-specs` (repo restricts branches to `geoff/*`).
- Spec source of truth: `docs/superpowers/specs/2026-06-23-trait-contract-specs-design.md`.

---

### Task 1: Emit spec fragments for `#[contract]` traits (core macro change)

Fully unit-testable at the token level — no artifact build required. Unchanged by the rv32 decision (the macro already emits the rv32 link-section arm; it just was never invoked for traits).

**Files:**
- Modify: `crates/macros/src/common/spec_reg.rs` (add `generate_trait_raw_spec_fragments`; add tests)
- Modify: `crates/macros/src/contract.rs:137` (wire the call into the trait branch; add a test)

**Interfaces:**
- Consumes (already defined in `crates/macros/src/common/`): `contract::ConfiguredContract::new`, `ContractSrc::methods_sorted`, `ContractSrc::defaulted_interface_vers`, `contract::effective_interface_name`, `contract::throws_spec_tokens`, `contract::throws_anchor_tokens`, `contract::method_anchor_tokens`, `ContractSrc for syn::ItemTrait`; and the in-module helpers `generate_interface_fragment_tokens`, `generate_method_fragment_tokens`, `generate_config_event_fragments_tokens`, `config_event_anchor_tokens`, `emit_root_anchor_tokens`.
- Produces: `pub fn spec_reg::generate_trait_raw_spec_fragments(item: &syn::ItemTrait, config: &Config) -> syn::parse::Result<proc_macro2::TokenStream>`. After this task, expanding `#[contract] trait T {…}` emits a `RAW_SPEC_FRAGMENT` static into `groundhogspecrawv0` under the gate cfg.

- [ ] **Step 1: Write the failing tests** — add to `#[cfg(test)] mod tests` in `crates/macros/src/common/spec_reg.rs`:

```rust
#[test]
fn trait_fragments_include_interface_and_method_records() {
    let item: syn::ItemTrait = syn::parse_quote! {
        pub trait ERC20 {
            fn transfer(&mut self, receiver: &sdk::ContractAddress, amount: u128);
            fn total_supply(&self) -> u128;
        }
    };
    let tokens = generate_trait_raw_spec_fragments(
        &item, &Config::from_list(&[]).unwrap(),
    ).unwrap().to_string();

    assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
    assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
    assert!(tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"), "{tokens}");
    assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
    assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
    assert!(tokens.contains("\"total_supply\""), "{tokens}");
    assert!(tokens.contains("\"transfer\""), "{tokens}");
    assert!(tokens.contains("\"receiver\""), "{tokens}");
}

#[test]
fn trait_fragments_carry_config_throws_and_emits() {
    let item: syn::ItemTrait = syn::parse_quote! {
        pub trait Token { fn transfer(&mut self); }
    };
    let config = parse_config(r#"throws(err::TokenError), emits(events::TransferEvent)"#);
    let tokens = generate_trait_raw_spec_fragments(&item, &config).unwrap().to_string();

    assert!(tokens.contains("ContractExceptionTypeView"), "{tokens}");
    assert!(
        tokens.contains("events :: TransferEvent as :: sdk :: __contract_spec :: ContractEventSpec > :: EVENT_DEF"),
        "{tokens}"
    );
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p groundhog-macros trait_fragments_`
Expected: FAIL — `cannot find function 'generate_trait_raw_spec_fragments'`.

- [ ] **Step 3: Implement** — add to `crates/macros/src/common/spec_reg.rs` after `generate_free_function_raw_spec_fragments` (~line 121):

```rust
/// Emit the `groundhogspecrawv0` fragments for a `#[contract]` trait
/// (interface) definition: one Interface fragment, one Method fragment per
/// trait method, event fragments, plus throws/event/signature anchor roots.
/// Mirrors the bare-impl path of `generate_impl_raw_spec_fragments`, but
/// methods come from the trait and there is no `self_ty`. Name/version match
/// the trait branch's `__GROUNDHOG_IFACE_*` consts, so a trait-derived spec is
/// byte-identical in format to the spec a concrete impl would carry.
pub fn generate_trait_raw_spec_fragments(
    item: &syn::ItemTrait,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();

    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    let interface_fragment = generate_interface_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let event_fragments = generate_config_event_fragments_tokens(config);

    let mut anchor_roots = throws_anchor_tokens(&contract)?;
    anchor_roots.extend(config_event_anchor_tokens(config));

    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                quote! { #iface_name },
                quote! { #iface_vers },
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        anchor_roots.extend(method_anchor_tokens(method)?);
    }
    let anchor_root =
        emit_root_anchor_tokens(&anchor_roots).unwrap_or_else(|| quote! {});

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
        #event_fragments
        #anchor_root
    })
}
```

The current `use super::contract::{…}` import already brings in every name used here — verify by compiling, add nothing speculatively.

- [ ] **Step 4: Run to verify pass** — `cargo test -p groundhog-macros trait_fragments_` → PASS.

- [ ] **Step 5: Wire into the trait branch + test** — in `crates/macros/src/contract.rs`, `syn::Item::Trait(item) =>` arm, just before `Ok(out)` (line 137):

```rust
            out.extend(spec_reg::generate_trait_raw_spec_fragments(
                item, config,
            )?);

            Ok(out)
```

Add to `#[cfg(test)] mod tests` in `contract.rs`:

```rust
    #[test]
    fn trait_contract_emits_raw_spec_fragments() {
        let item: syn::Item = syn::parse_quote! {
            pub trait ERC20 {
                fn transfer(&mut self, receiver: &sdk::ContractAddress, amount: u128);
                fn total_supply(&self) -> u128;
            }
        };
        let tokens = apply_with_config(&item, &parse_config("")).unwrap().to_string();
        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
        assert!(tokens.contains("pub static RAW_SPEC_FRAGMENT"), "{tokens}");
        assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
        assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"), "{tokens}");
    }
```

- [ ] **Step 6: Full macro suite** — `cargo test -p groundhog-macros` → PASS, no regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/macros/src/common/spec_reg.rs crates/macros/src/contract.rs
git commit -m "feat(macros): emit groundhogspecrawv0 fragments for #[contract] traits"
```

---

### Task 2: Add an ELF spec reader to `groundhog-spec-tool`

This closes the actual gap: rv32 emits fragments but nothing extracts them. The reader is target-agnostic glue over the existing canonicalizer, fully unit-testable with a synthetic ELF fixture (no rv32 toolchain needed) — mirroring how `tests/rewrite.rs` builds synthetic wasm.

**Files:**
- Modify: `crates/spec-tool/Cargo.toml` (add `object` dep)
- Create: `crates/spec-tool/src/elf.rs`
- Modify: `crates/spec-tool/src/lib.rs` (declare `pub mod elf;`, add `SpecToolError::Elf` variant, re-export)
- Modify: `crates/spec-tool/src/main.rs` (add a `spec --elf` subcommand)
- Test: `crates/spec-tool/tests/elf.rs`

**Interfaces:**
- Consumes: `raw::{RAW_SECTION_NAME, parse_raw_fragments}`, `canonicalize_spec(&[SpecFragment]) -> Result<ContractSpec, SpecToolError>`, `spec_to_json(&ContractSpec)` (all existing, exported from `lib.rs`); `groundhog_contract_spec::{SpecFragment, ContractSpec, SpecInterfaceFragment, SpecMethodFragment}` and `xdrs::txs::encode` (test fixture).
- Produces:
  - `pub fn elf::collect_elf_raw_section(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError>`
  - `pub fn elf::elf_to_spec(bytes: &[u8]) -> Result<Option<ContractSpec>, SpecToolError>`
  - new CLI: `groundhog-spec-tool spec --elf <PATH> --json-out <PATH> [--json-format debug|round-trip]`

- [ ] **Step 1: Add the `object` dependency** — in `crates/spec-tool/Cargo.toml` `[dependencies]`:

```toml
object = { version = "0.36", default-features = false, features = ["read_core", "elf", "std"] }
```

(If the workspace pins `object` in `[workspace.dependencies]`, use `object = { workspace = true }` instead. Check the root `Cargo.toml` first.)

- [ ] **Step 2: Add the `Elf` error variant** — in `crates/spec-tool/src/lib.rs`, add to the `SpecToolError` enum (next to the existing `Wasm` variant) and its `Display`/`Error` handling to match the existing pattern:

```rust
    /// An error occurred while parsing the ELF binary.
    Elf(String),
```

If `SpecToolError` derives `Debug`/`Clone`/`PartialEq` and renders via a hand-written `Display` or `thiserror`, follow whichever the file already uses (mirror the `Wasm(String)` variant exactly).

- [ ] **Step 3: Write the failing test** — create `crates/spec-tool/tests/elf.rs`. Build a minimal ELF carrying a `groundhogspecrawv0` section with one Interface + one Method fragment, then assert extraction + canonicalization:

```rust
use groundhog_contract_spec::{
    ContractField, ContractSpecString, ContractType, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_contract_spec::{ContractSpecMethod};
use groundhog_spec_tool::{elf::elf_to_spec, RAW_SECTION_NAME};
use object::write::{Object, StandardSection, SectionKind};
use object::{Architecture, BinaryFormat, Endianness};
use xdrs::txs::encode;

fn s(v: &str) -> ContractSpecString { ContractSpecString::try_from(v).unwrap() }

fn raw_fragments() -> Vec<u8> {
    let iface = SpecFragment::Interface(SpecInterfaceFragment {
        interface_name: s("ERC20"),
        interface_version: s("0.1.3"),
        throws: vec![],
    });
    let method = SpecFragment::Method(SpecMethodFragment {
        interface_name: s("ERC20"),
        interface_version: s("0.1.3"),
        method: ContractSpecMethod {
            name: s("total_supply"),
            args: vec![],
            return_type: ContractType::U128,
        },
    });
    let mut bytes = encode(&iface).unwrap();
    bytes.extend(encode(&method).unwrap());
    bytes
}

fn elf_with_spec_section(section_bytes: &[u8]) -> Vec<u8> {
    let mut obj = Object::new(
        BinaryFormat::Elf, Architecture::Riscv32, Endianness::Little,
    );
    let sec = obj.add_section(
        Vec::new(), RAW_SECTION_NAME.as_bytes().to_vec(), SectionKind::ReadOnlyData,
    );
    obj.set_section_data(sec, section_bytes.to_vec(), 1);
    obj.write().unwrap()
}

#[test]
fn elf_to_spec_extracts_interface_and_methods() {
    let elf = elf_with_spec_section(&raw_fragments());
    let spec = elf_to_spec(&elf).unwrap().expect("spec present");
    assert_eq!(spec.interfaces.len(), 1);
    let iface = &spec.interfaces[0];
    assert_eq!(std::str::from_utf8(&iface.interface_name).unwrap(), "ERC20");
    assert!(iface.methods.iter().any(|m|
        std::str::from_utf8(&m.name).unwrap() == "total_supply"));
}

#[test]
fn elf_without_spec_section_returns_none() {
    let elf = elf_with_spec_section(&[]);
    assert!(elf_to_spec(&elf).unwrap().is_none());
}
```

Add to `crates/spec-tool/Cargo.toml` `[dev-dependencies]` whatever the fixture needs that isn't already present: `object` (with the `write` feature) and `xdrs`. If `object` is a normal dependency without `write`, add a dev-dep line enabling it: `object = { version = "0.36", features = ["write", "read_core", "elf", "std"] }`. Confirm `groundhog-contract-spec` field names (`ContractSpec.interfaces`, `ContractSpecInterface.methods`, `ContractType::U128`) against `crates/contract-spec/src/lib.rs` before finalizing; adjust the asserts/fixture to the real types.

- [ ] **Step 4: Run to verify failure** — `cargo test -p groundhog-spec-tool --test elf`
Expected: FAIL — `elf` module / `elf_to_spec` not found.

- [ ] **Step 5: Implement** — create `crates/spec-tool/src/elf.rs`:

```rust
//! Extract contract-spec fragments from a riscv32 (or any) ELF's
//! `groundhogspecrawv0` section. The bytes are the same XDR fragment
//! concatenation the wasm path reads from a custom section, so they feed the
//! shared `parse_raw_fragments` + `canonicalize_spec` pipeline unchanged.

use groundhog_contract_spec::ContractSpec;
use object::{Object, ObjectSection};

use crate::{
    canonicalize_spec, parse_raw_fragments, SpecToolError, RAW_SECTION_NAME,
};

/// Concatenate the bytes of every `groundhogspecrawv0` section in `bytes`.
pub fn collect_elf_raw_section(bytes: &[u8]) -> Result<Vec<u8>, SpecToolError> {
    let file = object::File::parse(bytes)
        .map_err(|e| SpecToolError::Elf(e.to_string()))?;
    let mut raw = Vec::new();
    for section in file.sections() {
        let name =
            section.name().map_err(|e| SpecToolError::Elf(e.to_string()))?;
        if name == RAW_SECTION_NAME {
            let data = section
                .data()
                .map_err(|e| SpecToolError::Elf(e.to_string()))?;
            raw.extend_from_slice(data);
        }
    }
    Ok(raw)
}

/// Extract and canonicalize the contract spec from an ELF, or `None` if it
/// carries no spec fragments.
pub fn elf_to_spec(
    bytes: &[u8],
) -> Result<Option<ContractSpec>, SpecToolError> {
    let raw = collect_elf_raw_section(bytes)?;
    if raw.is_empty() {
        return Ok(None);
    }
    let fragments = parse_raw_fragments(&raw)?;
    match canonicalize_spec(&fragments) {
        Ok(spec) => Ok(Some(spec)),
        Err(SpecToolError::NoSpecFragments) => Ok(None),
        Err(err) => Err(err),
    }
}
```

In `crates/spec-tool/src/lib.rs`, add `pub mod elf;` next to the other `pub mod` declarations, and confirm `canonicalize_spec`, `parse_raw_fragments`, `RAW_SECTION_NAME` are reachable as written (they are `pub` per `lib.rs`/`raw.rs`; adjust the `use crate::{…}` path in `elf.rs` if any is re-exported under a different path).

- [ ] **Step 6: Run to verify pass** — `cargo test -p groundhog-spec-tool --test elf` → PASS (both tests).

- [ ] **Step 7: Add the CLI subcommand** — in `crates/spec-tool/src/main.rs`, add a `spec --elf <PATH> --json-out <PATH> [--json-format debug|round-trip]` subcommand that reads the ELF file, calls `groundhog_spec_tool::elf::elf_to_spec`, and writes JSON via `spec_to_json` (round-trip) or `render_spec_json` (debug) — mirroring how the `rewrite` arm writes its `--json-out`. Update the `USAGE` const with the new subcommand. If `elf_to_spec` returns `Ok(None)`, exit with a clear "no spec fragments found in ELF" error (mirror `SpecToolError::NoSpecFragments` handling).

- [ ] **Step 8: Verify the binary builds + usage renders** — `cargo run -q -p groundhog-spec-tool -- spec` (no args) → prints usage including the new `spec --elf` line; exit non-zero. `cargo build -p groundhog-spec-tool` → clean.

- [ ] **Step 9: Commit**

```bash
git add crates/spec-tool/Cargo.toml crates/spec-tool/src/elf.rs crates/spec-tool/src/lib.rs crates/spec-tool/src/main.rs crates/spec-tool/tests/elf.rs
git commit -m "feat(spec-tool): extract contract spec from rv32 ELF groundhogspecrawv0 section"
```

---

### Task 3: Build `groundhog-erc20-if` into an rv32 spec-carrier

Turns the ERC20 interface crate into a degenerate zero-entrypoint rv32 bin that carries its spec section. Uses the proven `extern crate <lib>;` wrapper pattern (`itc_erc20`). No cdylib, no wasm.

**Files:**
- Modify: `standard-interfaces/groundhog-erc20/Cargo.toml`
- Create: `standard-interfaces/groundhog-erc20/src/main.rs`

**Interfaces:**
- Consumes: Task 1 (trait emits fragments under `entrypoints`).
- Produces: `groundhog-erc20-if` builds via `cargo build -p groundhog-erc20-if --bin groundhog-erc20-if --features entrypoints --target riscv32im-unknown-none-elf` into an ELF with a non-empty `groundhogspecrawv0` section.

- [ ] **Step 1: Add the `entrypoints` feature** — in `standard-interfaces/groundhog-erc20/Cargo.toml` `[features]`:

```toml
[features]
# Empty per-crate switch read by the #[contract] macro via
# cfg!(feature = "entrypoints"). Mirrors example-contracts/erc20. Must NOT be
# enabled by dependents' entrypoints features.
entrypoints = []
dev = ["sdk/dev", "dep:thiserror"]
```

- [ ] **Step 2: Add the bin target** — append to `standard-interfaces/groundhog-erc20/Cargo.toml`. Keep `[lib] crate-type = ["rlib"]` unchanged (no cdylib):

```toml
[[bin]]
name = "groundhog-erc20-if"
path = "src/main.rs"
required-features = ["entrypoints"]
```

- [ ] **Step 3: Add the rv32 bin wrapper** — create `standard-interfaces/groundhog-erc20/src/main.rs`:

```rust
//! RV32 spec-carrier binary for the ERC20 standard interface.
//!
//! This is a degenerate, zero-entrypoint "contract": a trait registers no
//! dispatch methods, so the only thing this artifact carries is the ERC20
//! interface's `groundhogspecrawv0` fragments. It exists so the interface can
//! be built into an rv32 ELF and have its spec extracted with
//! `groundhog-spec-tool spec --elf`.
#![no_std]
#![no_main]

// `_start`, panic handler, allocator, and linker script come from the runtime
// crate (an unconditional `sdk` dependency).
extern crate groundhog_rv32_runtime;

// Link the interface library so its macro-emitted `#[used]` spec section is
// retained in the final ELF for extraction.
extern crate groundhog_erc20_if;
```

- [ ] **Step 4: Verify the host rlib still builds (no regression)** — `cargo build -p groundhog-erc20-if`
Expected: clean. Dependents consume the rlib and never enable `entrypoints`, so they are unaffected. (The `[[bin]]` is `required-features = ["entrypoints"]`, so it is skipped on normal host builds.)

- [ ] **Step 5: Build the rv32 spec-carrier ELF and confirm the section** — requires a nightly toolchain with `rust-src` and the `riscv32im-unknown-none-elf` target:

```bash
cargo build -p groundhog-erc20-if \
  --bin groundhog-erc20-if \
  --no-default-features --features entrypoints \
  --target riscv32im-unknown-none-elf --release \
  -Zbuild-std=core,alloc
ELF=target/riscv32im-unknown-none-elf/release/groundhog-erc20-if
# Confirm the retained ALLOC section is present:
rust-objdump -h "$ELF" | grep groundhogspecrawv0 \
  || llvm-objdump -h "$ELF" | grep groundhogspecrawv0
```

Expected: build succeeds; `groundhogspecrawv0` appears as a section. The `GROUNDHOG_RV32_ENTRYPOINTS` slice is empty (a trait registers no methods, `exports.rs:154-171`) — this is the intended zero-entrypoint carrier, and the v4 lowerer accepts a normal release build with no entrypoints.

If the link fails for a missing `_start`/lang item: drop the `extern crate groundhog_rv32_runtime;` line if the interface lib already pulls it transitively (mirror `itc_erc20/src/main.rs`, which lists only `extern crate itc_erc20;`), or vice-versa. Record the working form in the commit.

- [ ] **Step 6: Commit**

```bash
git add standard-interfaces/groundhog-erc20/Cargo.toml standard-interfaces/groundhog-erc20/src/main.rs
git commit -m "feat(erc20-if): build ERC20 interface into an rv32 spec-carrier bin"
```

---

### Task 4: End-to-end rv32 test — interface ELF yields the full ERC20 spec

Locks the whole chain: trait → rv32 ELF → `spec --elf` → canonical spec with every ERC20 method. Toolchain-gated; skips (passes) when the rv32 target is not installed, so CI without it does not fail.

**Files:**
- Create: `standard-interfaces/groundhog-erc20/tests/interface_spec_rv32.rs`
- Modify: `standard-interfaces/groundhog-erc20/Cargo.toml` (`[dev-dependencies]`)

**Interfaces:**
- Consumes: Task 1 + Task 2 + Task 3.

- [ ] **Step 1: Write the test** — create `standard-interfaces/groundhog-erc20/tests/interface_spec_rv32.rs`. It shells out (mirroring `crates/spec-tool/tests/rewrite.rs`'s `Command` usage), builds the rv32 ELF, then calls the in-repo `elf_to_spec`:

```rust
//! Builds the ERC20 interface as an rv32 spec-carrier and asserts the
//! extracted spec covers the whole interface. Requires a nightly toolchain
//! with the `riscv32im-unknown-none-elf` target + `rust-src`; otherwise skips.

use std::process::Command;

fn rv32_target_available() -> bool {
    Command::new("rustc")
        .args(["--print", "target-list"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout)
            .lines().any(|t| t == "riscv32im-unknown-none-elf"))
        .unwrap_or(false)
}

#[test]
fn erc20_interface_rv32_spec_is_complete() {
    if !rv32_target_available() {
        eprintln!("skipping: riscv32im-unknown-none-elf target not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let target_dir = dir.path().join("target");

    let status = Command::new(env!("CARGO"))
        .args([
            "build", "-p", "groundhog-erc20-if", "--bin", "groundhog-erc20-if",
            "--no-default-features", "--features", "entrypoints",
            "--target", "riscv32im-unknown-none-elf", "--release",
            "-Zbuild-std=core,alloc",
        ])
        .arg("--target-dir").arg(&target_dir)
        .status().unwrap();
    assert!(status.success(), "rv32 build failed");

    let elf = target_dir
        .join("riscv32im-unknown-none-elf/release/groundhog-erc20-if");
    let bytes = std::fs::read(&elf).unwrap();
    let spec = groundhog_spec_tool::elf::elf_to_spec(&bytes)
        .unwrap()
        .expect("ERC20 interface ELF must carry a spec");

    let iface = spec.interfaces.iter()
        .find(|i| std::str::from_utf8(&i.interface_name).unwrap() == "ERC20")
        .expect("ERC20 interface present");
    for method in [
        "initialize", "transfer", "transfer_from", "adjust_allowance",
        "allowance", "get_balance_of", "get_balance", "mint", "burn",
        "total_supply",
    ] {
        assert!(
            iface.methods.iter().any(|m|
                std::str::from_utf8(&m.name).unwrap() == method),
            "missing method {method}");
    }
}
```

- [ ] **Step 2: Add dev-deps** — in `standard-interfaces/groundhog-erc20/Cargo.toml` `[dev-dependencies]`, add `tempfile` and a path dep on the spec tool so the test can call `elf_to_spec`:

```toml
[dev-dependencies]
groundhog-erc20-if = { path = ".", features = ["dev"] }
tempfile = { workspace = true }
groundhog-spec-tool = { workspace = true }
```

If `tempfile` / `groundhog-spec-tool` are not workspace deps, pin directly (`tempfile = "3"`, `groundhog-spec-tool = { path = "../../crates/spec-tool" }`). Confirm against the root `Cargo.toml`.

- [ ] **Step 3: Run the test** — `cargo test -p groundhog-erc20-if --test interface_spec_rv32 -- --nocapture`
Expected: PASS if the rv32 target is installed (spec extracted, all methods present), else printed skip + PASS.
If FAIL with no spec / missing method: the gap is in Task 1 (emission), Task 2 (extraction), or Task 3 (Cargo wiring) — fix there, not in the test.

- [ ] **Step 4: Commit**

```bash
git add standard-interfaces/groundhog-erc20/tests/interface_spec_rv32.rs standard-interfaces/groundhog-erc20/Cargo.toml
git commit -m "test(erc20-if): assert rv32 interface ELF yields a complete ERC20 spec"
```

---

### Task 5: CLI handoff note

**Files:**
- Create: `docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md`

- [ ] **Step 1: Write the note**

```markdown
# CLI note: building interface specs on the rv32 path

After the sdk_rs changes, building a spec from an interface is fully rv32-native
and wasm-free:

1. `cargo build -p groundhog-erc20-if --bin groundhog-erc20-if \
     --no-default-features --features entrypoints \
     --target riscv32im-unknown-none-elf --release -Zbuild-std=core,alloc`
2. `groundhog-spec-tool spec --elf <ELF> --json-out erc20.json`

`spec-tool` now reads the `groundhogspecrawv0` section directly from the ELF
(`groundhog_spec_tool::elf::elf_to_spec`) — `rv32-lower` is NOT required for
spec extraction (it only packages the deploy artifact and drops the spec from VM
memory).

**For `thruput build --target rv32` (groundhog-rpc):** this is where the rv32
spec extraction belonged and appears to be missing. Wire the rv32 build path to
call `groundhog_spec_tool::elf::elf_to_spec` on the freshly built ELF and emit
the spec JSON sidecar, exactly as the wasm path emits its spec. No new
extraction logic is needed in the CLI — depend on `groundhog-spec-tool` and call
the new `elf` module. A trait-only interface compiles to a zero-entrypoint
contract, so relax any "must have ≥1 entrypoint" guard for the interface case.

Ergonomic optional add: `groundhog spec-interface -p <crate>` that runs the rv32
build + `elf_to_spec` and writes the JSON in one step.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md
git commit -m "docs: CLI handoff note for rv32 interface spec extraction"
```

---

## Notes on scope

- Implements **ERC20 only**. Extend to `groundhog-erc1271` / `groundhog-erc20-metadata` by repeating Tasks 3–4 per crate; the macro (Task 1) and the ELF reader (Task 2) cover all interfaces at once.
- The shared emission gate keeps the `target_family = "wasm"` arm; fully removing wasm support is a separate future change, out of scope here.
- Conformance checking (impl-vs-interface) and an interface registry remain out of scope.
```
