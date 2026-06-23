# Building contract specs from `#[contract]` traits

**Date:** 2026-06-23
**Status:** Approved design, pending implementation plan

## Problem

Today the `#[contract]` macro only emits spec fragments (the XDR-encoded
`groundhogspecrawv0` link-section data) for **impl blocks** and **free
functions**. When `#[contract]` is applied to a **trait** (an interface
definition such as `groundhog-erc20-if::ERC20`), it emits only Rust metadata
consts (`__GROUNDHOG_IFACE_NAME`, `__GROUNDHOG_IFACE_THROWS`,
`__GROUNDHOG_IFACE_EVENTS`, anchor-root fns) plus the client and the
`XEntrypoints` ID-constant struct. It never calls `spec_reg`, so the trait
contributes **no spec fragment bytes** on its own — it is purely a metadata
donor that concrete impls reference via `<Self as Trait>::__GROUNDHOG_IFACE_*`.

We want to produce a canonical contract spec **from the interface trait alone**,
with no concrete impl. Concretely:

- **Goal 1 — publish a canonical interface spec.** Produce an authoritative,
  standalone spec artifact for the standard interfaces (ERC20 first) that others
  can build and verify against.
- **Goal 2 — generate clients/bindings from the interface.** Drive
  client/off-chain binding generation from the interface spec, decoupled from
  any deployed contract.

Constraints from the user:

- Must work for the **rv32 (`riscv32im-unknown-none-elf`) target** — that is the
  primary target going forward. A wasm-only solution is not acceptable.
- Should be **driven by the groundhog CLI**. The CLI may be extended; ergonomics
  over the long term is the priority.

## Key findings (current state)

1. **The trait branch never emits fragments.** `crates/macros/src/contract.rs`
   `apply_with_config`:
   - Trait branch (`:40-137`): emits metadata consts + client +
     `generate_entrypoints`. No `spec_reg` call.
   - Impl branch (`:139-207`): calls
     `spec_reg::generate_impl_raw_spec_fragments` (`:184`).
   - Free-fn branch (`:208-232`): calls
     `spec_reg::generate_free_function_raw_spec_fragments` (`:219`).

2. **Fragment helpers are reusable and literal-driven.** In
   `crates/macros/src/common/spec_reg.rs`, `generate_interface_fragment_tokens`,
   `generate_method_fragment_tokens`, `generate_config_event_fragments_tokens`,
   and the anchor-root helpers operate on expansion-time data. The trait branch
   already computes everything they need: `iface_name`, `iface_vers`,
   `throws_spec_tokens`, the `emits` list, and the trait's method signatures (via
   the existing `ContractSrc` / `Method` impl for `syn::ItemTrait`). No `self_ty`
   indirection is required for a standalone trait.

3. **A trait registers zero dispatch entrypoints.** `exports.rs`
   `generate_entrypoints` (`:154-171`) only produces the `XEntrypoints`
   ID-constant struct. The actual `extern "C"` wrappers and the
   `GROUNDHOG_RV32_ENTRYPOINTS` distributed-slice registrations come from
   `generate_method`, which is only invoked from `generate()` in the impl /
   free-fn branches. Therefore an interface compiled as a contract is a **valid
   "empty" program** that dispatches nothing but can still carry the spec section.

4. **Spec emission and extraction are same-crate, feature-gated.** Spec statics
   are emitted with `#[cfg(all(any(target_family = "wasm", target_arch =
   "riscv32"), feature = "entrypoints"))]` and retained on rv32 via `#[used]`
   (`spec_reg.rs:154-289`). The `entrypoints` feature is the local crate's
   feature, declared per crate (`example-contracts/erc20/Cargo.toml:15`) and
   passed by the build as `--features entrypoints`. The statics live in whatever
   crate is compiled to the artifact.

5. **Extraction pipeline already exists per target.**
   - wasm: `groundhog build` → `groundhog-spec-tool rewrite --wasm`
     (`parse_raw_fragments` → canonicalize → `spec_to_json`).
   - rv32: `cargo build --features entrypoints --target riscv32im-...` →
     published `rv32-lower` extracts the `#[used] groundhogspecrawv0` section.

6. **Interface crates are bare rlibs today.**
   `standard-interfaces/groundhog-erc20/Cargo.toml`: `crate-type = ["rlib"]`,
   `test = false`, no `entrypoints` feature — so they currently produce no
   artifact and no spec section.

## Approach

Make the **interface crate itself** the spec-carrying artifact, reusing the
existing extraction pipeline end-to-end. An interface compiled with
`entrypoints` is a degenerate, zero-method contract that carries only its spec
fragments.

This was chosen over two alternatives considered during design:

- **Host-side `linkme` registry + ephemeral harness crate** (rejected): emit
  fragments into a host-visible distributed slice and canonicalize on the host.
  Target-neutral, but adds new SDK runtime surface (a registry), a new host spec
  path, and a second way specs are produced that must be kept consistent with
  the artifact path. More code, more divergence, no format benefit.
- **Separate carrier crate depending on the interface** (rejected): relies on
  cross-crate `#[used]`/`link_section` retention from a dependency rlib into the
  final artifact, which is linker-dependent and historically flaky. Putting the
  artifact target in the interface crate keeps the spec statics in the same
  compilation unit as the artifact, matching today's contract crates exactly.

Net: one extraction path to maintain, and the interface-derived spec is identical
in format to what a concrete rv32/wasm contract implementing the trait carries
(both go through the same `spec_reg` helpers and the same canonicalizer).

## Changes

### 1. `crates/macros` — emit trait spec fragments (core fix)

- Add `spec_reg::generate_trait_raw_spec_fragments(item: &syn::ItemTrait, config:
  &Config) -> Result<TokenStream>`. It emits the `Interface` fragment, one
  `Method` fragment per trait method, and the event fragments, reusing the
  existing helpers:
  - interface name/version/throws from the values the trait branch already
    computes (`iface_name`, `iface_vers`, `throws_spec_tokens`);
  - method fragments from the trait's method signatures via the existing
    `ContractSrc` / `Method` impl for `ItemTrait`;
  - event fragments from the `emits(...)` list (reuse
    `generate_config_event_fragments_tokens` / `config_event_anchor_tokens`);
  - anchor roots for throws/events/method signatures, emitted via the existing
    `emit_root_anchor_tokens`.
  - Gated on the same `cfg(all(any(wasm, riscv32), feature = "entrypoints"))` as
    impls (reuse `emit_raw_fragment_tokens`).
- Wire the call into the **trait branch** of `contract.rs` (`apply_with_config`,
  `syn::Item::Trait`), alongside the existing `generate_entrypoints` call near
  `:135`.
- No new feature flag. Reuse `entrypoints`.

### 2. `standard-interfaces/*` Cargo.toml

For `groundhog-erc20`, `groundhog-erc1271`, `groundhog-erc20-metadata`:

- Add an `entrypoints = []` feature (passthrough enabled by the build), mirroring
  `example-contracts/erc20/Cargo.toml:15`.
- Add `crate-type = ["cdylib", "rlib"]` so the crate can be compiled to a wasm
  cdylib / rv32 artifact while remaining usable as an rlib dependency.
- A trivial `[[bin]]` may be required if the rv32 flow demands `--bin` rather
  than building the cdylib; decide during implementation against the actual rv32
  build invocation.

### 3. CLI (external; blurb only)

Expected to be near-zero. Once changes 1–2 land,
`groundhog build -p groundhog-erc20-if --features entrypoints` should flow
through the existing wasm-rewrite / rv32-lower extraction unchanged and emit the
spec JSON.

The one plausible CLI change: an interface compiles to a contract with **zero**
dispatch entrypoints. If `groundhog build` or `rv32-lower` asserts "at least one
entrypoint," that guard must be relaxed or gated behind an explicit
`--interface` / `--allow-empty` flag. A short note will be handed to whoever owns
the CLI repo describing exactly this.

## Open risk to validate during implementation

An empty, spec-only contract (zero entrypoints) must:

1. Link and lower cleanly on rv32 (`_start` scans an empty
   `GROUNDHOG_RV32_ENTRYPOINTS` slice), and
2. Not be rejected by any "must have entrypoints" guard in the CLI or lowerer.

Everything else reuses proven paths. This risk should be retired early with a
spike: add the Cargo changes to one interface crate, build it for rv32 with
`--features entrypoints`, and confirm the artifact carries the spec section and
lowers.

## Testing

- **Macro unit tests** (mirror the existing `trait_contract_*` tests in
  `contract.rs` / `spec_reg.rs`): assert a `#[contract] trait` now emits
  `SpecInterfaceFragmentView` + `SpecMethodFragmentView` tokens and the
  `groundhogspecrawv0` link-section/`#[used]` machinery.
- **Integration**: build `groundhog-erc20-if` as an artifact with
  `--features entrypoints`, extract its spec, and assert the JSON equals the
  **interface portion** of the spec extracted from the existing
  `example-contracts/erc20` impl-derived artifact. This locks the
  "identical by construction" guarantee between trait-derived and impl-derived
  specs.

## Out of scope

- Conformance checking (verifying an impl matches an interface spec).
- An interface registry / catalog.
- Changes to the wasm/rv32 extraction or canonicalization logic itself.
