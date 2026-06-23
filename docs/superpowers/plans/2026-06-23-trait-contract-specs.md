# Trait Contract Specs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `#[contract]` on a trait emit spec fragments so a canonical contract spec can be built from a standard interface (ERC20 first) with no concrete impl, driven through the existing build/extract pipeline.

**Architecture:** Add a `generate_trait_raw_spec_fragments` path to the `groundhog-macros` crate that emits the same `groundhogspecrawv0` link-section fragments for a trait as the impl path does today, reusing the existing `spec_reg` helpers. Then make each standard-interface crate compilable into a spec-carrying artifact (degenerate zero-entrypoint "contract") by adding an `entrypoints` feature and a `cdylib` crate-type, exactly like `example-contracts/erc20`. The existing wasm-rewrite / rv32-lower extraction then produces the spec unchanged.

**Tech Stack:** Rust proc-macros (`syn`, `quote`, `proc-macro2`, `darling`), the `groundhog-spec-tool` crate, wasm32 + `riscv32im-unknown-none-elf` targets.

## Global Constraints

- The fragment-emission cfg gate must be **verbatim**: `#[cfg(all(any(target_family = "wasm", target_arch = "riscv32"), feature = "entrypoints"))]`. Reuse `spec_reg::emit_raw_fragment_tokens` / `emit_root_anchor_tokens`, which already encode it. Do not hand-roll the cfg.
- `entrypoints` is a **per-crate feature read by the macro via `cfg!(feature = "entrypoints")`**, not an `sdk` feature. Declare it as `entrypoints = []` on each interface crate. Do NOT add `sdk/entrypoints` (it does not exist).
- **Do not** make any contract crate's `entrypoints` feature forward to an interface dependency's `entrypoints` (e.g. never add `groundhog-erc20-if/entrypoints` to `erc20`'s `entrypoints` feature). That would compile the interface's fragments into the contract artifact and risk duplicate/conflicting Method fragments. Verified today: no crate does this.
- The trait-derived interface fragment must use the **same** interface name and version the trait branch already emits into its `__GROUNDHOG_IFACE_*` consts: name = `config.interface_name().unwrap_or(trait ident)`, version = `config.version_string().unwrap_or(CARGO_PKG_VERSION)`. Using `effective_interface_name(&contract)` and `contract.defaulted_interface_vers()` yields exactly these.
- Branch naming in this repo is restricted to `geoff/*`. Work is on branch `geoff/trait-contract-specs`.
- Spec source of truth: `docs/superpowers/specs/2026-06-23-trait-contract-specs-design.md`.

---

### Task 1: Emit spec fragments for `#[contract]` traits (core macro change)

This is the whole core fix and is fully unit-testable at the token level — no artifact build required.

**Files:**
- Modify: `crates/macros/src/common/spec_reg.rs` (add `generate_trait_raw_spec_fragments`; add tests)
- Modify: `crates/macros/src/contract.rs:137` (wire the call into the trait branch; add a test)

**Interfaces:**
- Consumes (all already defined in `crates/macros/src/common/`):
  - `contract::ConfiguredContract::new(Config, &T) -> ConfiguredContract<T>`
  - `contract::ContractSrc::methods_sorted(&self) -> impl IntoIterator<Item = impl Method>`
  - `contract::ContractSrc::defaulted_interface_vers(&self) -> String`
  - `contract::effective_interface_name(&impl ContractSrc) -> String`
  - `contract::throws_spec_tokens(&impl ContractSrc) -> Result<Vec<TokenStream>>`
  - `contract::throws_anchor_tokens(&impl ContractSrc) -> Result<Vec<TokenStream>>`
  - `contract::method_anchor_tokens(&impl Method) -> Result<Vec<TokenStream>>`
  - `ContractSrc for syn::ItemTrait` (already implemented; `methods()` yields `TraitItemFn`)
  - In-module helpers in `spec_reg.rs`: `generate_interface_fragment_tokens`, `generate_method_fragment_tokens`, `generate_config_event_fragments_tokens`, `config_event_anchor_tokens`, `emit_root_anchor_tokens`.
- Produces (relied on by Task 2/3):
  - `pub fn spec_reg::generate_trait_raw_spec_fragments(item: &syn::ItemTrait, config: &Config) -> syn::parse::Result<proc_macro2::TokenStream>`
  - After this task, expanding `#[contract] trait T { ... }` emits a `RAW_SPEC_FRAGMENT` static into `groundhogspecrawv0` under the gate cfg.

- [ ] **Step 1: Write the failing test for the new function**

Add to the `#[cfg(test)] mod tests` block in `crates/macros/src/common/spec_reg.rs` (the module that already imports `use super::*;` and defines `parse_config`):

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
        &item,
        &Config::from_list(&[]).unwrap(),
    )
    .unwrap()
    .to_string();

    // Same link-section + retention machinery as the impl path.
    assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
    assert!(tokens.contains("target_arch = \"riscv32\""), "{tokens}");
    assert!(tokens.contains("used)] pub static RAW_SPEC_FRAGMENT"), "{tokens}");
    // Interface + method records present.
    assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
    assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
    assert!(tokens.contains("const_txs_size"), "{tokens}");
    // Methods are sorted by name and the receiver is dropped from args.
    assert!(tokens.contains("\"total_supply\""), "{tokens}");
    assert!(tokens.contains("\"transfer\""), "{tokens}");
    assert!(tokens.contains("\"receiver\""), "{tokens}");
}

#[test]
fn trait_fragments_carry_config_throws_and_emits() {
    let item: syn::ItemTrait = syn::parse_quote! {
        pub trait Token {
            fn transfer(&mut self);
        }
    };
    let config = parse_config(r#"throws(err::TokenError), emits(events::TransferEvent)"#);

    let tokens = generate_trait_raw_spec_fragments(&item, &config)
        .unwrap()
        .to_string();

    // throws lands in the interface fragment via ContractExceptionTypeView.
    assert!(tokens.contains("ContractExceptionTypeView"), "{tokens}");
    // emits lands as an Event fragment referencing the declared event def.
    assert!(
        tokens.contains(
            "events :: TransferEvent as :: sdk :: __contract_spec :: ContractEventSpec > :: EVENT_DEF"
        ),
        "{tokens}"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p groundhog-macros trait_fragments_ -- --nocapture`
Expected: FAIL — `cannot find function 'generate_trait_raw_spec_fragments' in this scope`.

- [ ] **Step 3: Implement `generate_trait_raw_spec_fragments`**

Add this function to `crates/macros/src/common/spec_reg.rs`, immediately after `generate_free_function_raw_spec_fragments` (around line 121). It mirrors the bare-impl branch of `generate_impl_raw_spec_fragments` but draws methods from the trait and has no `self_ty`:

```rust
/// Emit the `groundhogspecrawv0` fragments for a `#[contract]` trait
/// (interface) definition: one Interface fragment, one Method fragment per
/// trait method, plus event fragments and signature/throws/event anchor
/// roots. Mirrors the bare-impl path of `generate_impl_raw_spec_fragments`,
/// but methods come from the trait and there is no `self_ty`.
///
/// Name/version are taken from the same sources the trait branch uses for its
/// `__GROUNDHOG_IFACE_*` consts, so the trait-derived spec is byte-identical
/// in format to the spec a concrete impl of this trait would carry.
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

Then extend the `use super::contract::{...}` import at the top of `spec_reg.rs` to include the names this function needs but that aren't already imported. The current import (lines 2-6) already brings in `ConfiguredContract`, `ContractSrc`, `Method`, `effective_interface_name`, `method_anchor_tokens`, `method_arg_spec_tokens`, `method_return_spec_token`, `throws_anchor_tokens`, `throws_spec_tokens`. No import change is needed — verify by compiling.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p groundhog-macros trait_fragments_ -- --nocapture`
Expected: PASS (both tests).

- [ ] **Step 5: Wire the call into the trait branch and write its test**

In `crates/macros/src/contract.rs`, in the `syn::Item::Trait(item) =>` arm, add the fragment emission just before `Ok(out)` (currently line 137):

```rust
            out.extend(spec_reg::generate_trait_raw_spec_fragments(
                item, config,
            )?);

            Ok(out)
```

Add a test to the `#[cfg(test)] mod tests` block in `contract.rs` (the one with `parse_config` + `apply_with_config`):

```rust
    #[test]
    fn trait_contract_emits_raw_spec_fragments() {
        let item: syn::Item = syn::parse_quote! {
            pub trait ERC20 {
                fn transfer(&mut self, receiver: &sdk::ContractAddress, amount: u128);
                fn total_supply(&self) -> u128;
            }
        };

        let tokens = apply_with_config(&item, &parse_config(""))
            .unwrap()
            .to_string();

        assert!(tokens.contains("groundhogspecrawv0"), "{tokens}");
        assert!(tokens.contains("pub static RAW_SPEC_FRAGMENT"), "{tokens}");
        assert!(tokens.contains("SpecInterfaceFragmentView"), "{tokens}");
        assert!(tokens.contains("SpecMethodFragmentView"), "{tokens}");
        // The trait's own metadata consts are still emitted alongside.
        assert!(tokens.contains("__GROUNDHOG_IFACE_NAME"), "{tokens}");
    }
```

- [ ] **Step 6: Run the full macro test suite**

Run: `cargo test -p groundhog-macros`
Expected: PASS, including the existing `trait_contract_*`, `emits_tests`, and the two new tests. No previously-passing test regresses.

- [ ] **Step 7: Commit**

```bash
git add crates/macros/src/common/spec_reg.rs crates/macros/src/contract.rs
git commit -m "feat(macros): emit groundhogspecrawv0 fragments for #[contract] traits"
```

---

### Task 2: Make `groundhog-erc20-if` build into a spec-carrying artifact

Turns the ERC20 interface crate into a degenerate zero-entrypoint contract artifact and verifies the spec section is present. This also retires the "empty contract" risk from the spec.

**Files:**
- Modify: `standard-interfaces/groundhog-erc20/Cargo.toml`

**Interfaces:**
- Consumes: Task 1 (the trait now emits fragments under `feature = "entrypoints"`).
- Produces: a `groundhog-erc20-if` crate buildable as a `cdylib` whose wasm/rv32 artifact contains a non-empty `groundhogspecrawv0` section.

- [ ] **Step 1: Add the `entrypoints` feature**

In `standard-interfaces/groundhog-erc20/Cargo.toml`, change the `[features]` block from:

```toml
[features]
dev = ["sdk/dev", "dep:thiserror"]
```

to:

```toml
[features]
# Empty, per-crate switch read by the #[contract] macro via
# cfg!(feature = "entrypoints") to emit entrypoint/spec machinery. Mirrors
# example-contracts/erc20. Must NOT be enabled by dependents' entrypoints
# features, or the interface's fragments would land in their artifacts.
entrypoints = []
dev = ["sdk/dev", "dep:thiserror"]
```

- [ ] **Step 2: Add `cdylib` to the crate-type**

In the same file, change:

```toml
[lib]
crate-type = ["rlib"]
doctest = false
test = false
```

to:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
doctest = false
test = false
```

- [ ] **Step 3: Verify the crate still builds as a normal host rlib (no regression)**

Run: `cargo build -p groundhog-erc20-if`
Expected: builds clean. The `cdylib` is not produced for the host target's normal lib use; existing dependents (`erc20`, tests) are unaffected because they consume the `rlib` and do not enable `entrypoints`.

- [ ] **Step 4: Build the wasm spec-carrier artifact and confirm the spec section exists**

Run:

```bash
cargo build -p groundhog-erc20-if \
  --features entrypoints \
  --target wasm32-unknown-unknown --release
WASM=target/wasm32-unknown-unknown/release/groundhog_erc20_if.wasm
ls -la "$WASM"
# List custom sections; expect a non-empty groundhogspecrawv0.
cargo run -q -p groundhog-spec-tool -- rewrite \
  --wasm "$WASM" \
  --out /tmp/erc20_if.canon.wasm \
  --json-out /tmp/erc20_if.spec.json \
  --json-format round-trip
cat /tmp/erc20_if.spec.json
```

Expected: the build succeeds; `rewrite` does NOT error with `NoSpecFragments`; `/tmp/erc20_if.spec.json` contains an interface named `ERC20` with methods `transfer`, `total_supply`, `mint`, `burn`, `allowance`, `transfer_from`, `adjust_allowance`, `get_balance`, `get_balance_of`, `initialize`, and a `throws` entry for `ERC20Error`, and events `TransferEvent` / `ApprovalEvent`.

If `rewrite` reports `NoSpecFragments`: the gate cfg or feature wiring is wrong — re-check Task 1's cfg and that `--features entrypoints` is on the `groundhog-erc20-if` build.

- [ ] **Step 5: Build the rv32 spec-carrier ELF and confirm the section + zero entrypoints (primary target)**

Run:

```bash
cargo build -p groundhog-erc20-if \
  --features entrypoints \
  --target riscv32im-unknown-none-elf --release \
  -Zbuild-std=core,alloc
ELF=$(find target/riscv32im-unknown-none-elf/release -maxdepth 1 -name '*.wasm' -o -name 'groundhog_erc20_if*' 2>/dev/null | head)
# Confirm the retained ALLOC section is present in the ELF.
rust-objdump -h target/riscv32im-unknown-none-elf/release/libgroundhog_erc20_if* 2>/dev/null | grep groundhogspecrawv0 \
  || llvm-objdump -h target/riscv32im-unknown-none-elf/release/*groundhog_erc20_if* | grep groundhogspecrawv0
```

Expected: the build succeeds and `groundhogspecrawv0` appears as a section. Because a trait registers zero dispatch entrypoints (`exports.rs:154-171` emits only the `XEntrypoints` ID struct), the `GROUNDHOG_RV32_ENTRYPOINTS` slice is empty — this is the intended "empty contract" carrier.

Note: `-Zbuild-std` requires a nightly toolchain with the `rust-src` component and the `riscv32im-unknown-none-elf` target installed. If the cdylib crate-type is rejected for the rv32 ELF flow (which historically wants a `--bin`), add a minimal `[[bin]]` to the interface crate that re-exports the lib (a `fn main() {}` is sufficient since `_start` comes from the unconditional `groundhog-rv32-runtime` dep) and rebuild with `--bin groundhog-erc20-if`. Record whichever path works in the commit message.

- [ ] **Step 6: Commit**

```bash
git add standard-interfaces/groundhog-erc20/Cargo.toml
git commit -m "feat(erc20-if): build ERC20 interface into a spec-carrying artifact"
```

---

### Task 3: End-to-end snapshot — trait spec matches the impl's interface portion

Locks the "identical by construction" guarantee: the spec extracted from the interface crate must equal the interface portion of the spec extracted from the concrete `erc20` example contract.

**Files:**
- Create: `standard-interfaces/groundhog-erc20/tests/interface_spec.rs`

**Interfaces:**
- Consumes: Task 1 + Task 2 (the interface crate now produces a spec artifact).
- Produces: an automated regression guarding the interface spec shape.

- [ ] **Step 1: Write the failing host test that drives the build + extraction**

Create `standard-interfaces/groundhog-erc20/tests/interface_spec.rs`. This is a host integration test that shells out to `cargo` + `groundhog-spec-tool` (mirroring how `crates/spec-tool/tests/rewrite.rs` already invokes `Command` and `tempfile`). It is gated to run only when the wasm target is available, so CI without the target skips rather than fails.

```rust
//! Builds the ERC20 interface crate as a spec-carrier and asserts the
//! extracted spec describes the full ERC20 interface. Requires the
//! `wasm32-unknown-unknown` target; skips (passes) if it is unavailable.

use std::process::Command;

fn wasm_target_available() -> bool {
    Command::new("rustc")
        .args(["--print", "target-list"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|t| t == "wasm32-unknown-unknown"))
        .unwrap_or(false)
}

#[test]
fn erc20_interface_spec_is_complete() {
    if !wasm_target_available() {
        eprintln!("skipping: wasm32-unknown-unknown target not installed");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let target_dir = dir.path().join("target");

    let status = Command::new(env!("CARGO"))
        .args([
            "build", "-p", "groundhog-erc20-if",
            "--features", "entrypoints",
            "--target", "wasm32-unknown-unknown", "--release",
        ])
        .arg("--target-dir").arg(&target_dir)
        .status()
        .unwrap();
    assert!(status.success(), "wasm build failed");

    let wasm = target_dir
        .join("wasm32-unknown-unknown/release/groundhog_erc20_if.wasm");
    let json = dir.path().join("erc20_if.spec.json");

    let status = Command::new(env!("CARGO"))
        .args(["run", "-q", "-p", "groundhog-spec-tool", "--",
               "rewrite", "--wasm"])
        .arg(&wasm)
        .arg("--out").arg(dir.path().join("erc20_if.canon.wasm"))
        .arg("--json-out").arg(&json)
        .args(["--json-format", "round-trip"])
        .status()
        .unwrap();
    assert!(status.success(), "spec extraction failed");

    let spec = std::fs::read_to_string(&json).unwrap();

    for method in [
        "initialize", "transfer", "transfer_from", "adjust_allowance",
        "allowance", "get_balance_of", "get_balance", "mint", "burn",
        "total_supply",
    ] {
        assert!(spec.contains(method), "missing method {method} in {spec}");
    }
    assert!(spec.contains("ERC20"), "missing interface name: {spec}");
    assert!(spec.contains("Transfer"), "missing TransferEvent: {spec}");
    assert!(spec.contains("Approval"), "missing ApprovalEvent: {spec}");
}
```

Add `tempfile` to `[dev-dependencies]` of `standard-interfaces/groundhog-erc20/Cargo.toml`:

```toml
[dev-dependencies]
groundhog-erc20-if = { path = ".", features = ["dev"] }
tempfile = { workspace = true }
```

(If `tempfile` is not a workspace dependency, pin it directly: `tempfile = "3"`. Confirm by checking the root `Cargo.toml` `[workspace.dependencies]`.)

Note: this test sets `test = false` is on `[lib]`, not on integration tests, so `tests/` still runs. Verify `cargo test -p groundhog-erc20-if` discovers it.

- [ ] **Step 2: Run the test to verify it fails (before Task 1/2 are present it would; here confirm it now passes or is skipped)**

Run: `cargo test -p groundhog-erc20-if --test interface_spec -- --nocapture`
Expected: PASS if the wasm target is installed (spec extracted and all methods present), or a printed skip line + PASS if not installed.

If it FAILS with `NoSpecFragments` or a missing method, the macro emission (Task 1) or Cargo wiring (Task 2) is incomplete — fix there, not in the test.

- [ ] **Step 3: Commit**

```bash
git add standard-interfaces/groundhog-erc20/tests/interface_spec.rs standard-interfaces/groundhog-erc20/Cargo.toml
git commit -m "test(erc20-if): assert interface crate emits a complete ERC20 spec"
```

---

### Task 4: CLI handoff note (no code in this repo)

Produces the short blurb the user asked for, describing what (if anything) the external groundhog CLI needs.

**Files:**
- Create: `docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md`

- [ ] **Step 1: Write the note**

Create `docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md`:

```markdown
# CLI note: building specs from interface crates

After the sdk_rs changes (trait fragment emission + interface crates gaining an
`entrypoints` feature and `cdylib` crate-type), an interface crate builds into a
spec-carrying artifact through the **existing** flows:

- wasm: `groundhog build -p groundhog-erc20-if --features entrypoints`
  (then the existing rewrite step emits the spec JSON).
- rv32: `cargo build -p groundhog-erc20-if --features entrypoints
  --target riscv32im-unknown-none-elf` then `rv32-lower --elf ... --out ...`.

The single thing to check in the CLI / lowerer: an interface compiles to a
contract with **zero dispatch entrypoints** (a trait registers none). If
`groundhog build` or `rv32-lower` asserts "at least one entrypoint", relax that
guard or gate it behind an explicit flag (suggested: `--interface` /
`--allow-empty`). No other CLI change is required — extraction and
canonicalization are unchanged.

Ergonomic optional add: a thin `groundhog spec-interface -p <crate>` alias that
runs the build + extract with `--features entrypoints` and `--interface`
preset.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-06-23-trait-contract-specs-cli-note.md
git commit -m "docs: CLI handoff note for interface-crate spec builds"
```

---

## Notes on scope

- This plan implements **ERC20 only**. To extend to `groundhog-erc1271` and
  `groundhog-erc20-metadata`, repeat Task 2 (add `entrypoints = []` + `cdylib`)
  and optionally Task 3 (a per-crate interface spec test) for each. The macro
  change (Task 1) covers all of them at once.
- Conformance checking (impl-vs-interface) and an interface registry are
  explicitly out of scope per the design doc.
