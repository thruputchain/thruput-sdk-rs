# Event Spec Shaking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Events declared via `emits(...)` on unimplemented interface traits must be shaken out of the canonical spec, the same way throws/methods/typedefs already are; as a prerequisite, the raw-spec fragment types move from protocol-rs's `groundhog-spec` into this repo's `groundhog-contract-spec`, making the raw wire format SDK-internal.

**Architecture:** The raw `groundhogspecrawv0` section is a concatenation of XDR `SpecFragment` values. Interfaces/methods/throws shake correctly because the interface fragment carries `throws` inline and method fragments carry an interface key; event fragments are anonymous defs, so `canonicalize_spec` keeps all of them. Fix: add an `emits` name list to the interface fragment (references into the global event-def pool, mirroring how throws reference typedefs), and make canonicalization retain only event defs referenced by surviving (anchored) interfaces plus `ExportedSpecEvent` defs. Since this changes the fragment wire format, first relocate the owned fragment types (`SpecFragment`, `SpecInterfaceFragment`, `SpecMethodFragment`, `SpecInterfaceAnchor`) into `groundhog-contract-spec` so protocol-rs is untouched.

**Tech Stack:** Rust workspace; `xdrs` XDR derives (`ReadXDR`/`WriteXDR`/`ConstXDRSize` for owned types, `ConstWriteXDR`/`WriteXDR` for views); proc macros in `crates/macros`; post-processing in `crates/spec-tool`.

## Global Constraints

- **Branch:** a repo hook blocks commits on `main`. Run `git checkout -b geoff/event-spec-shaking` as its own command (not chained with `&&`) before the first commit.
- **No new dependencies.** Everything needed (`xdrs`, `groundhog-spec`) is already a dependency of the crates being touched.
- **Do not modify protocol-rs** (`../../protocol-rs`). Removing the fragment section from `protocol/groundhog_contract_spec.x` + xdrc regen is a separate follow-up in that repo.
- **Section names unchanged:** `groundhogspecrawv0`, `groundhogspecanchorsv0`, `groundhogspecanchortextv0`. A spec-tool test (`crates/spec-tool/src/raw.rs` tests) guards the link_section literals in macros — do not rename.
- **Wire field order:** the new `emits` field goes **after** `throws` in `SpecInterfaceFragment` (owned) and `SpecInterfaceFragmentView` (view). XDR structs are order-sensitive; owned and view field order must match exactly.
- **Formatting:** run `cargo fmt` before each commit. Match existing comment density/style.
- **Known pre-existing failure:** `erc20_low_level` can flake in full-suite runs — not caused by this work.
- Workspace tests are run from `/Users/geofframseyer/groundhog/agent1/sdk_rs`.

---

### Task 1: Move owned raw-fragment types into `groundhog-contract-spec`

Pure refactor, no wire-format change. The owned fragment types currently come from protocol-rs's generated `groundhog-spec` crate; only `contract-spec` (view→owned `From` impls), `spec-tool`, and their tests use them, and `groundhog-rpc` only reaches them through `groundhog_spec_tool` re-exports. `SpecFragmentKind` is referenced nowhere except the re-export line — drop it.

**Files:**
- Create: `crates/contract-spec/src/fragment.rs`
- Modify: `crates/contract-spec/src/lib.rs:10-19` (re-export list)
- Tests: existing `crates/contract-spec/tests/roundtrip.rs` and `crates/spec-tool/tests/*` are the regression harness (they encode views / hand-built owned fragments and decode them; byte-compat failure = test failure).

**Interfaces:**
- Consumes: `groundhog_spec::{ContractEventDef, ContractExceptionType, ContractSpecMethod, ContractSpecString, ContractTypeDef}` (these STAY in protocol-rs), `xdrs` derives.
- Produces: `groundhog_contract_spec::{SpecFragment, SpecInterfaceFragment, SpecMethodFragment, SpecInterfaceAnchor}` with identical shapes/discriminants as before — all downstream imports keep working unchanged.

- [ ] **Step 1: Create the branch**

```bash
git checkout -b geoff/event-spec-shaking
```

- [ ] **Step 2: Write `crates/contract-spec/src/fragment.rs`**

Copy the shapes from `../protocol-rs/groundhog-spec/src/groundhog_contract_spec.rs:525-648`, hand-written (no serde/schemars — nothing serializes fragments as JSON; no `SpecFragmentKind` companion struct — use literal discriminants):

```rust
//! Owned raw-spec fragment types.
//!
//! `groundhogspecrawv0` fragments are internal to the SDK toolchain: the
//! `#[contract]` macros emit them (as const views) and spec-tool parses and
//! canonicalizes them. Downstream consumers only ever see the finalized
//! `ContractSpec`, so the fragment types live here rather than in the
//! protocol-level `groundhog-spec` crate.

use groundhog_spec::{
    ContractEventDef, ContractExceptionType, ContractSpecMethod,
    ContractSpecString, ContractTypeDef,
};
use xdrs::*;

#[derive(ReadXDR, WriteXDR, ConstXDRSize)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecInterfaceFragment {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
    pub throws: VecM<ContractExceptionType>,
}

#[derive(ReadXDR, WriteXDR, ConstXDRSize)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecMethodFragment {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
    pub method: ContractSpecMethod,
}

#[derive(ReadXDR, WriteXDR, ConstXDRSize)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecInterfaceAnchor {
    pub interface_name: ContractSpecString,
    pub interface_version: ContractSpecString,
}

#[derive(ReadXDR, WriteXDR, ConstXDRSize)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum SpecFragment {
    SpecInterface { interface_fragment: SpecInterfaceFragment } = 0,
    SpecMethod { method_fragment: SpecMethodFragment } = 1,
    SpecTypeDef { type_def: ContractTypeDef } = 2,
    ExportedSpecTypeDef { type_def: ContractTypeDef } = 3,
    SpecEvent { event_def: ContractEventDef } = 4,
    ExportedSpecEvent { event_def: ContractEventDef } = 5,
    InterfaceAnchor { interface_anchor: SpecInterfaceAnchor } = 6,
}
```

(If the `ReadXDR` derive needs extra derives or attributes the generated code carried — e.g. `#[repr(C)]` on structs — the compile error will name them; mirror the generated file. The generated file's `#[non_exhaustive]` is intentionally dropped: we own both sides now.)

- [ ] **Step 3: Rewire `crates/contract-spec/src/lib.rs`**

In the `#[cfg(feature = "owned-model")] pub use groundhog_spec::{...}` block (lines 10-19), delete `SpecFragment`, `SpecFragmentKind`, `SpecInterfaceAnchor`, `SpecInterfaceFragment`, `SpecMethodFragment`. Below the block add:

```rust
#[cfg(feature = "owned-model")]
mod fragment;
#[cfg(feature = "owned-model")]
pub use fragment::*;
```

- [ ] **Step 4: Verify byte-compat via existing tests**

Run: `cargo test -p groundhog-contract-spec -p groundhog-spec-tool`
Expected: PASS (roundtrip and canonicalize tests exercise encode/decode of the moved types; any derive/shape mismatch fails here).

- [ ] **Step 5: Verify nothing still needs the old exports**

Run: `cargo build --workspace && grep -rn "groundhog_spec::Spec" crates/ --include="*.rs"`
Expected: clean build; grep finds nothing.

- [ ] **Step 6: Commit**

```bash
git add crates/contract-spec/src/fragment.rs crates/contract-spec/src/lib.rs
git commit -m "contract-spec: move raw spec fragment types in from groundhog-spec

Raw groundhogspecrawv0 fragments are internal to the SDK toolchain; only
the finalized ContractSpec is protocol-level. Owning the fragment types
here lets the raw format evolve without touching protocol-rs."
```

---

### Task 2: Emit `__GROUNDHOG_IFACE_EMITS` names const on `#[contract]` traits

A trait impl in another crate builds its interface fragment from the trait's `__GROUNDHOG_IFACE_*` consts (it cannot re-derive the emits list — the `emits(...)` attribute lives on the trait). Add a names const next to the existing `__GROUNDHOG_IFACE_EVENTS` defs const.

**Files:**
- Modify: `crates/macros/src/contract.rs` (trait-processing branch, after the `__GROUNDHOG_IFACE_EVENTS` push at ~line 114-117; the `emits` Vec of `syn::Path` from config is already in scope — see its use at line 61)
- Test: `crates/macros/src/contract.rs` in-file tests (existing tests asserting `__GROUNDHOG_IFACE_EVENTS` are at lines ~411, ~442)

**Interfaces:**
- Consumes: `ContractEventSpec::EVENT_DEF` (`crates/contract-spec/src/lib.rs`), whose `.name` field is `&'static str`.
- Produces: trait associated const `__GROUNDHOG_IFACE_EMITS: &'static [&'static str]`, in `emits(...)` declaration order — Task 4's trait-impl path references `<Self as Trait>::__GROUNDHOG_IFACE_EMITS`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/macros/src/contract.rs`, next to the existing test that asserts `__GROUNDHOG_IFACE_EVENTS` (find it via the assertion at line ~411 and mirror its setup — it parses a `#[contract]` trait with an `emits(...)` config and stringifies the output tokens):

```rust
#[test]
fn trait_gets_emits_names_const() {
    // Same setup as the neighboring __GROUNDHOG_IFACE_EVENTS test:
    // a trait processed with config `emits(events::TransferEvent)`.
    // (Copy that test's parse/config/expand boilerplate verbatim.)
    assert!(tokens.contains("__GROUNDHOG_IFACE_EMITS"), "{tokens}");
    assert!(
        tokens.contains(
            "ContractEventSpec > :: EVENT_DEF . name"
        ),
        "{tokens}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-sdk-macros trait_gets_emits_names_const` (adjust `-p` to the macros crate's real package name from `crates/macros/Cargo.toml` if different)
Expected: FAIL on the `__GROUNDHOG_IFACE_EMITS` assertion.

- [ ] **Step 3: Implement**

In `crates/macros/src/contract.rs`, immediately after the `stripped.items.push` for `__GROUNDHOG_IFACE_EVENTS` (line ~114-117):

```rust
let emits_name_tokens = emits.iter().map(|path| {
    quote::quote! {
        <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF.name
    }
});
stripped.items.push(parse_quote! {
    #[doc(hidden)]
    const __GROUNDHOG_IFACE_EMITS: &'static [&'static str] = &[#(#emits_name_tokens),*];
});
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p <macros-package>`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/macros/src/contract.rs
git commit -m "macros: emit __GROUNDHOG_IFACE_EMITS names const on #[contract] traits"
```

---

### Task 3: Add `emits` to the interface fragment (view + owned + conversions)

Wire-format change: `SpecInterfaceFragment` gains `emits` (event-name references) after `throws`. This task also mechanically updates every existing literal construction so the workspace compiles; the emission and shaking behavior land in Tasks 4-5.

**Files:**
- Modify: `crates/contract-spec/src/view.rs:178-182` (`SpecInterfaceFragmentView`), `:555-570` (`From<SpecInterfaceFragmentView> for SpecInterfaceFragment`)
- Modify: `crates/contract-spec/src/fragment.rs` (from Task 1)
- Modify (mechanical `emits:` additions to struct literals): `crates/contract-spec/tests/roundtrip.rs`, `crates/spec-tool/tests/canonicalize.rs` (~23 sites), `crates/spec-tool/tests/describe.rs`, `crates/spec-tool/tests/json_roundtrip.rs`, `crates/spec-tool/tests/rewrite.rs` — wherever `SpecInterfaceFragment` / `SpecInterfaceFragmentView` literals appear (let the compiler enumerate).
- Test: `crates/contract-spec/tests/roundtrip.rs`

**Interfaces:**
- Produces: `SpecInterfaceFragmentView.emits: &'a [&'a str]` (Task 4 constructs it) and `SpecInterfaceFragment.emits: VecM<ContractSpecString>` (Task 5 reads it). View and owned field order: `interface_name`, `interface_version`, `throws`, `emits`.

- [ ] **Step 1: Write the failing test**

In `crates/contract-spec/tests/roundtrip.rs`, add (using the file's existing `s()` helper and its established encode-view/decode-owned pattern — mirror the neighboring fragment roundtrip test's structure exactly):

```rust
#[test]
fn interface_fragment_emits_roundtrips() {
    let view = SpecFragmentView::Interface(SpecInterfaceFragmentView {
        interface_name: "Token",
        interface_version: "0.1.0",
        throws: &[],
        emits: &["TransferEvent", "ApprovalEvent"],
    });
    let encoded = txs::encode(&view).unwrap();
    let (decoded, _): (SpecFragment, _) =
        xdrs::txs::buf_reader::decode(&encoded).unwrap();
    // ^ use the same decode call the existing view->owned roundtrip tests use
    let SpecFragment::SpecInterface { interface_fragment } = decoded else {
        panic!("wrong variant");
    };
    assert_eq!(
        interface_fragment.emits.to_vec(),
        vec![s("TransferEvent"), s("ApprovalEvent")]
    );
    // Owned conversion path agrees:
    let owned = SpecFragment::from(view);
    assert_eq!(owned, SpecFragment::SpecInterface { interface_fragment });
}
```

(Adapt the encode/decode helper calls to whatever the existing roundtrip tests in that file use — `xdrs::cxs::{encode, decode}` or `txs` — keep consistent with neighbors.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-contract-spec interface_fragment_emits_roundtrips`
Expected: FAIL to compile — no `emits` field.

- [ ] **Step 3: Add the fields and conversion**

`crates/contract-spec/src/view.rs:178`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
pub struct SpecInterfaceFragmentView<'a> {
    pub interface_name: &'a str,
    pub interface_version: &'a str,
    pub throws: &'a [ContractExceptionTypeView<'a>],
    /// Names of the events this interface declares via `emits(...)`.
    /// References into the artifact's event-def pool; the post-processor
    /// retains only event defs referenced by a surviving interface (or
    /// force-exported via `#[event_spec(export)]`).
    pub emits: &'a [&'a str],
}
```

`crates/contract-spec/src/fragment.rs` — add after `throws`:

```rust
    pub emits: VecM<ContractSpecString>,
```

`crates/contract-spec/src/view.rs` `From` impl (line ~555) — add after the `throws` mapping:

```rust
            emits: value
                .emits
                .iter()
                .copied()
                .map(to_spec_string)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
```

- [ ] **Step 4: Fix all literal constructions workspace-wide**

Run: `cargo build --workspace --tests` and add `emits: &[]` (views) / `emits: vec![].try_into().unwrap()` (owned) to every site the compiler reports. Expected sites: the test files listed above. Do NOT touch `crates/macros` here (its generated tokens change in Task 4; macros crate itself still compiles since it only emits token strings).

- [ ] **Step 5: Run tests**

Run: `cargo test -p groundhog-contract-spec -p groundhog-spec-tool`
Expected: PASS, including the new roundtrip test.

- [ ] **Step 6: Commit**

```bash
git add -A crates/contract-spec crates/spec-tool
git commit -m "contract-spec: interface fragments carry emits event-name refs

Events were the only interface-owned spec data with no tie back to the
interface: defs landed in a global pool with nothing referencing them,
so spec shaking could not drop events of unimplemented interfaces.
Mirror the throws/typedef split: refs on the interface fragment,
defs in the pool."
```

---

### Task 4: Thread `emits` through the macro fragment-emission sites

All four interface-fragment generation sites gain the emits list: trait impls read the trait's `__GROUNDHOG_IFACE_EMITS` const; trait definitions, bare impls, and free functions build names from the `emits(...)` config via `EVENT_DEF.name`.

**Files:**
- Modify: `crates/macros/src/common/spec_reg.rs` — `generate_interface_fragment_tokens` (line ~433) and its four call sites (lines ~25, ~66, ~118, ~177); in-file tests.

**Interfaces:**
- Consumes: `__GROUNDHOG_IFACE_EMITS` (Task 2), `SpecInterfaceFragmentView.emits` (Task 3), `Config::emits_list()` (existing).
- Produces: raw interface fragments whose `emits` byte-content is identical between a trait's candidate fragment and its impl's fragment (both derive from the trait's `emits(...)` config in declaration order — required so duplicate fragments dedupe/agree in Task 5's conflict check).

- [ ] **Step 1: Write the failing tests**

In `crates/macros/src/common/spec_reg.rs` tests:

Extend `trait_fragments_carry_config_throws_and_emits` (line ~760):

```rust
        assert!(
            tokens.contains("EVENT_DEF . name"),
            "interface fragment must carry emits names: {tokens}"
        );
```

Extend `trait_impl_fragments_include_interface_and_method_records` (line ~557):

```rust
        assert!(tokens.contains("__GROUNDHOG_IFACE_EMITS"), "{tokens}");
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p <macros-package> spec_reg`
Expected: the two extended tests FAIL on the new assertions.

- [ ] **Step 3: Implement**

In `crates/macros/src/common/spec_reg.rs`:

Add a helper next to `config_event_anchor_tokens` (line ~421):

```rust
/// Event-name reference tokens for the `emits` field of an interface
/// fragment, from the `emits(...)` config list, in declaration order.
fn config_emits_name_tokens(config: &Config) -> Vec<TokenStream> {
    config
        .emits_list()
        .into_iter()
        .map(|path| {
            quote! {
                <#path as ::sdk::__contract_spec::ContractEventSpec>::EVENT_DEF.name
            }
        })
        .collect()
}
```

Change `generate_interface_fragment_tokens` (line ~433) to take and emit the new field:

```rust
fn generate_interface_fragment_tokens(
    cfg: &TokenStream,
    interface_name: TokenStream,
    interface_version: TokenStream,
    throws: TokenStream,
    emits: TokenStream,
) -> TokenStream {
    emit_raw_fragment_tokens_with_cfg(
        cfg.clone(),
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::Interface(
                ::sdk::__contract_spec::SpecInterfaceFragmentView {
                    interface_name: #interface_name,
                    interface_version: #interface_version,
                    throws: #throws,
                    emits: #emits,
                }
            )
        },
    )
}
```

Update the four call sites:

1. Trait impl (line ~25): pass `quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_EMITS }`.
2. Bare impl (line ~66), 3. free function (line ~118), 4. trait candidate (line ~177): in each, before the call add

```rust
    let emits_names = config_emits_name_tokens(config);
```

and pass `quote! { &[#(#emits_names),*] }`.

(For the bare-impl and free-function paths `config` is the parameter of that function; for the trait path it is likewise in scope — see the existing `generate_config_event_fragments_tokens(&target_cfg(), config)` calls beside each site.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p <macros-package>`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/macros/src/common/spec_reg.rs
git commit -m "macros: emit emits name refs on raw interface fragments"
```

---

### Task 5: Shake events in `canonicalize_spec`

Retain an event def only if a surviving (anchored) interface's `emits` references it, or it was force-exported via `#[event_spec(export)]`. Conflict-check event defs only among retained ones — unimplemented interfaces in one interface crate must not be able to fail an unrelated contract's build. Error if a surviving interface emits a name with no def in the artifact.

**Files:**
- Modify: `crates/spec-tool/src/lib.rs` — `SpecToolError` (line ~40) + `Display` (line ~64), `InterfaceBucket` (line ~100), `canonicalize_spec` (lines ~133-320)
- Test: `crates/spec-tool/tests/canonicalize.rs`

**Interfaces:**
- Consumes: `SpecInterfaceFragment.emits: VecM<ContractSpecString>` (Task 3).
- Produces: `SpecToolError::{ConflictingInterfaceEmits, MissingEventDef}` variants; `ContractSpec.events` now contains only retained defs, sorted by name (BTreeSet iteration order — same ordering as today's BTreeMap).

- [ ] **Step 1: Write the failing tests**

Add to `crates/spec-tool/tests/canonicalize.rs`, using its existing `s()` helper and `canon()`/`canonicalize_spec` patterns. Helper + tests:

```rust
fn event(name: &str) -> groundhog_contract_spec::ContractEventDef {
    groundhog_contract_spec::ContractEventDef {
        name: s(name),
        signature: [0u8; 32].into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    }
}

fn iface_with_emits(name: &str, emits: &[&str]) -> SpecFragment {
    SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s(name),
            interface_version: s("0.1.0"),
            throws: vec![].try_into().unwrap(),
            emits: emits.iter().map(|e| s(e)).collect::<Vec<_>>().try_into().unwrap(),
        },
    }
}

fn anchor(name: &str) -> SpecFragment {
    SpecFragment::InterfaceAnchor {
        interface_anchor: SpecInterfaceAnchor {
            interface_name: s(name),
            interface_version: s("0.1.0"),
        },
    }
}

#[test]
fn events_of_unanchored_interfaces_are_shaken() {
    // Impl'd interface A emits AEvent; interface B is an unimplemented
    // candidate from the same interface crate emitting BEvent. Only AEvent
    // may survive.
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &["AEvent"]),
        SpecFragment::SpecEvent { event_def: event("AEvent") },
        iface_with_emits("B", &["BEvent"]),
        SpecFragment::SpecEvent { event_def: event("BEvent") },
        anchor("A"),
    ])
    .unwrap();
    let names: Vec<&str> =
        spec.events.iter().map(|e| spec_str(&e.name)).collect();
    assert_eq!(names, vec!["AEvent"]);
}

#[test]
fn exported_events_survive_without_any_interface_reference() {
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        SpecFragment::ExportedSpecEvent { event_def: event("LooseEvent") },
        anchor("A"),
    ])
    .unwrap();
    let names: Vec<&str> =
        spec.events.iter().map(|e| spec_str(&e.name)).collect();
    assert_eq!(names, vec!["LooseEvent"]);
}

#[test]
fn conflicting_defs_ignored_when_event_is_shaken() {
    // B's conflicting def of "Ev" must not fail A's build once B is shaken.
    let mut other = event("Ev");
    other.signature = [1u8; 32].into();
    let spec = canonicalize_spec(&[
        iface_with_emits("A", &[]),
        iface_with_emits("B", &["Ev"]),
        SpecFragment::SpecEvent { event_def: event("Ev") },
        SpecFragment::SpecEvent { event_def: other },
        anchor("A"),
    ])
    .unwrap();
    assert!(spec.events.is_empty());
}

#[test]
fn conflicting_defs_of_retained_event_still_error() {
    let mut other = event("Ev");
    other.signature = [1u8; 32].into();
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["Ev"]),
        SpecFragment::SpecEvent { event_def: event("Ev") },
        SpecFragment::SpecEvent { event_def: other },
        anchor("A"),
    ])
    .unwrap_err();
    assert!(matches!(err, SpecToolError::ConflictingEventDef(_)), "{err}");
}

#[test]
fn emits_reference_without_def_errors() {
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["GhostEvent"]),
        anchor("A"),
    ])
    .unwrap_err();
    assert!(matches!(err, SpecToolError::MissingEventDef(_)), "{err}");
}

#[test]
fn conflicting_interface_emits_error() {
    let err = canonicalize_spec(&[
        iface_with_emits("A", &["Y"]),
        iface_with_emits("A", &["X"]),
        SpecFragment::SpecEvent { event_def: event("X") },
        SpecFragment::SpecEvent { event_def: event("Y") },
        anchor("A"),
    ])
    .unwrap_err();
    assert!(
        matches!(err, SpecToolError::ConflictingInterfaceEmits(_)),
        "{err}"
    );
}
```

(`ContractEventDef.signature`'s exact type comes from `groundhog-spec` — if `[0u8; 32].into()` doesn't compile, construct it the way `crates/contract-spec/tests/roundtrip.rs` constructs event signatures.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p groundhog-spec-tool --test canonicalize`
Expected: new tests FAIL (`events_of_unanchored_interfaces_are_shaken` sees both events; `ConflictingInterfaceEmits`/`MissingEventDef` don't exist yet → compile error first; add the enum variants (Step 3a) before the behavioral steps if needed to get failing-not-erroring tests).

- [ ] **Step 3: Implement**

3a. Error variants in `crates/spec-tool/src/lib.rs` (enum at line ~40, `Display` at ~64):

```rust
    /// Multiple interface fragments declare different `emits` for the same interface.
    ConflictingInterfaceEmits(String),
    /// An interface's `emits` references an event with no definition fragment.
    MissingEventDef(String),
```

```rust
            Self::ConflictingInterfaceEmits(msg) => {
                write!(f, "conflicting emits metadata for interface {msg}")
            }
            Self::MissingEventDef(msg) => {
                write!(f, "missing event definition for {msg}")
            }
```

3b. `InterfaceBucket` (line ~100):

```rust
struct InterfaceBucket {
    throws: Option<Vec<ContractExceptionType>>,
    emits: Option<Vec<ContractSpecString>>,
    methods: Vec<ContractSpecMethod>,
}
```

3c. In the `SpecFragment::SpecInterface` arm (line ~138-171), after the throws handling, canonicalize + record emits (sorted + deduped, so fragment ordering can't cause spurious conflicts):

```rust
                let mut emits = fragment.emits.to_vec();
                for name in &emits {
                    if spec_str(name)?.is_empty() {
                        return Err(SpecToolError::InvalidRawFragment(
                            "interface emits entries must be non-empty"
                                .to_string(),
                        ));
                    }
                }
                emits.sort();
                emits.dedup();
                match &bucket.emits {
                    Some(existing) if *existing != emits => {
                        return Err(SpecToolError::ConflictingInterfaceEmits(
                            interface_key,
                        ));
                    }
                    Some(_) => {}
                    None => bucket.emits = Some(emits),
                }
```

(Note: `interface_key` is consumed by the existing throws conflict arm — rework so both arms can use it, e.g. clone or reorder; keep the throws behavior identical.)

3d. Track exported events separately. Split the event-collection arm (line ~213-220):

```rust
            SpecFragment::SpecEvent { event_def }
            | SpecFragment::ExportedSpecEvent { event_def } => {
                validate_event(event_def)?;
                if matches!(fragment, SpecFragment::ExportedSpecEvent { .. }) {
                    exported_events.insert(event_def.name.clone());
                }
                let defs = events.entry(event_def.name.clone()).or_default();
                if !defs.iter().any(|existing| existing == event_def) {
                    defs.push(event_def.clone());
                }
            }
```

with `let mut exported_events = std::collections::BTreeSet::<ContractSpecString>::new();` declared beside `events` (line ~133). Update the now-stale comment above the arm: SpecEvent defs are candidates referenced by interface `emits` lists; ExportedSpecEvent forces inclusion.

3e. Delete the pre-shaking events computation (lines ~247-257). In the interface filter/map (line ~262), collect retained names — the map closure gains the emits handling next to the existing `bucket.throws` unwrap (line ~293):

```rust
    let mut retained_event_names = exported_events;
    let interfaces = interfaces
        .into_iter()
        .filter(|((name, vers), _)| {
            anchored_interfaces.contains(&(name.clone(), vers.clone()))
        })
        .map(|((name, vers), mut bucket)| {
            // ... existing method sort/dedup and throws unwrap unchanged ...
            let emits = bucket.emits.ok_or_else(|| {
                SpecToolError::MissingInterfaceFragment(interface_key_string(
                    &spec_str_lossy(&name),
                    &spec_str_lossy(&vers),
                ))
            })?;
            retained_event_names.extend(emits);
            Ok(ContractSpecInterface { /* unchanged */ })
        })
        .collect::<Result<Vec<_>, _>>()?;
```

3f. After the interfaces block, build the final events list (replaces the deleted block; note `events` map must be `mut` now):

```rust
    // Event shaking: a def survives only if a surviving interface's `emits`
    // references it, or it was force-exported via `#[event_spec(export)]`.
    // Conflicts among shaken defs are deliberately ignored — an unimplemented
    // interface candidate must not fail an unrelated contract's build.
    let events = retained_event_names
        .into_iter()
        .map(|name| {
            let defs = events.remove(&name).ok_or_else(|| {
                SpecToolError::MissingEventDef(
                    spec_str_lossy(&name).into_owned(),
                )
            })?;
            if defs.len() > 1 {
                return Err(SpecToolError::ConflictingEventDef(
                    spec_str_lossy(&name).into_owned(),
                ));
            }
            Ok(defs.into_iter().next().unwrap())
        })
        .collect::<Result<Vec<_>, _>>()?;
```

`resolve_reachable_type_defs(&interfaces, &events, ...)` (line ~309) stays as-is and now only walks retained events' field types — shaking their typedefs automatically.

3g. Update the stale comment at line ~208-212 (done in 3d) and the doc comment on `ContractEventSpec` in `crates/contract-spec/src/lib.rs` if it still says defs land unconditionally.

- [ ] **Step 4: Run tests**

Run: `cargo test -p groundhog-spec-tool`
Expected: PASS — new tests plus all existing canonicalize/describe/json/rewrite tests (existing tests use empty `emits` lists, so their events... note: existing tests that expect events in output must reference them via `emits` now, or via `ExportedSpecEvent`. If an existing test constructs `SpecEvent` fragments and expects them retained, update that test's interface fragment to list the event names in `emits` — that is the new contract, mirroring what the macros now emit.)

- [ ] **Step 5: Commit**

```bash
git add crates/spec-tool/src/lib.rs crates/spec-tool/tests/canonicalize.rs
git commit -m "spec-tool: shake unreferenced event defs in canonicalize_spec

Events of interface-trait candidates that nothing anchors are now
dropped, matching the existing interface/method/throws/typedef shaking.
Exported events (#[event_spec(export)]) still land unconditionally.
Conflict-checking now applies only to retained defs."
```

---

### Task 6: Full-workspace verification and end-to-end check

**Files:** none new — verification only (plus any fixes it forces).

- [ ] **Step 1: Format and lint**

Run: `cargo fmt --all && cargo clippy --workspace --tests`
Expected: clean (fix anything introduced by Tasks 1-5).

- [ ] **Step 2: Full test suite**

Run: `cargo test --workspace`
Expected: PASS. Known exception: `erc20_low_level` full-suite flake is pre-existing — rerun that test alone before attributing it to this change.

- [ ] **Step 3: End-to-end spec check on a real contract**

The original bug shape is "interface crate with many traits, impl crate implements a subset". `example-contracts/swap` declares interfaces in `src/interfaces/` — build it for a contract target and inspect the emitted spec the way the repo's existing integration tests do (see `crates/sdk/tests/events.rs` and `crates/spec-tool/tests/` for the established build+extract pattern). Verify:
- events of implemented interfaces appear in the final spec;
- no events from unimplemented interface traits appear.

If no existing example exercises the subset case, extend `example-contracts/spec_named_types` (or the sdk integration tests) with a second, unimplemented `#[contract]` trait carrying `emits(...)` and assert its event is absent from the canonicalized spec.

- [ ] **Step 4: Commit any verification fixes, then push and open PR**

```bash
git push -u origin geoff/event-spec-shaking
gh pr create --title "spec: shake events of unimplemented interface traits" --body "..."
```

Then use the `check-ci` skill to watch CI and address review feedback.

---

### Task 7: Version bump

The raw-format change is breaking for readers (`groundhog-rpc` pins `groundhog-spec-tool`/`groundhog-contract-spec` from the registry, currently 0.17.7; old readers cannot parse new artifacts' raw sections). Bump the workspace version on this branch so the release carrying this change is distinguishable.

**Executed by the controller session directly** (not a subagent): invoke this workspace's `bump-version` skill and follow it — it encodes the bump size policy, the files to touch (workspace version, template pins, lockfiles), and the version-check tooling (`--ignore-lockfiles` etc.). Commit per that skill's conventions, then verify with the repo's version-check and `cargo build --workspace`.

---

## Out of Scope (follow-ups, separate PRs)

1. **protocol-rs cleanup:** delete the "Spec fragments" section from `protocol/groundhog_contract_spec.x` and regenerate `groundhog-spec` via xdrc (the fragment types now live in sdk_rs).
2. **groundhog-rpc pin update:** after this releases, update groundhog-rpc's registry pins so its extraction tooling parses new artifacts (final `groundhogspecv0` sections are unaffected).
3. **Optional symmetry:** consider carrying the event signature (topic 0) alongside the name in `emits` entries, the way throws carry selectors — only if a consumer materializes that needs it.
