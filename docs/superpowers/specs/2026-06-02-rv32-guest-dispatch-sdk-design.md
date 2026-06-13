# RV32 Guest-Side Dispatch — SDK Support

Status: design (2026-06-02). Approved direction; ready for an implementation plan.

This spec covers the **SDK-side** work needed so a Groundhog contract built for
`riscv32im-unknown-none-elf` dispatches its own methods from `_start`, rather
than relying on the runtime to route `method_id → PC` through an artifact
dispatch table. It is the `sdk_rs` slice of the broader v4 cut described in the
runtime-side design doc (guest-side dispatch / artifact v4). The lowerer
(`groundhog-rpc`) and runtime (`hogd`) changes are **out of scope here** but
their contract with this work is pinned in [Cross-repo contract](#cross-repo-contract).

## Goal

1. `_start` (in `groundhog-rv32-runtime`) reads the invoked `method_id` from
   registers and dispatches to the right method wrapper.
2. The dispatch table is generated **ergonomically, exactly like the existing
   native (mock) dispatch** — each `#[contract]` impl self-registers its
   methods; no manual enumeration; multiple interfaces on one contract type all
   register and collect automatically.
3. The rv32 method-export symbols (`#[export_name = "<16HEX>"]` and the
   `.text.gh_*` link sections) are removed — `_start` is the sole rv32 dispatch
   surface. WASM is unaffected and keeps host-side export dispatch.

## Background: what exists today

- **WASM** dispatches host-side: the macro emits `#[export_name = "<16HEX>"]`
  per method, and hogd calls `instance.get_func(method)`. **Unchanged by this
  work.**
- **RV32 (v3)** dispatches runtime-side: the macro additionally pins each method
  into `.text.gh_<16HEX>` so the linker keeps the symbol; the lowerer scans
  `.symtab`, builds a sorted `method_id → PC` table into the artifact, and hogd
  enters at the selected PC. `_start` is a trapping stub
  (`crates/rv32-runtime/src/start.rs`) that today only runs if dispatch routes
  method 0 to it.
- **Native (mock)** dispatch already uses the ergonomic model we want: a
  `linkme::distributed_slice` named `CONTRACT_MOCK_FRAGMENTS`
  (`crates/sdk/src/mock/mock_registry.rs`). Each `#[contract]` impl emits a
  `#[distributed_slice]` static (`crates/macros/src/common/test_reg.rs`) that
  registers `(entrypoint_id, fn)`; the runtime iterates the slice. This is the
  template for guest dispatch.

Dependency direction: `sdk` → `groundhog-rv32-runtime` (target-gated;
`crates/sdk/src/lib.rs` force-links it with `extern crate … as _`).
`groundhog-rv32-runtime` is intentionally dependency-minimal and ships the rv32
linker script via its `build.rs`.

## Feasibility — proven by spike

A standalone `no_std` rv32 binary (custom linker script matching ours, release +
LTO + `strip = "symbols"` + `--gc-sections`, `-Z trap-unreachable=false`)
confirmed:

1. **linkme works on bare-metal rv32.** Three `#[distributed_slice]`
   registrations from "different impls" collected into one contiguous, bounded
   table; the linker-generated `__start`/`__stop` bounds resolved to **link-time
   immediates**, so the table survives a fully stripped binary (the v4
   "strip-works" goal). The consumer loop read the bounds as constants.
2. **The `a0`/`a1` → `u64` ABI is free.** A `_start` that preserves `a0`/`a1`
   and calls `extern "C" fn dispatch(method_id: u64)` reassembles the seeded
   halves automatically: the RISC-V C ABI passes a `u64` as `a0`=low / `a1`=high,
   which is exactly the runtime-side seeding contract. No manual capture or
   shift/or needed.
3. **No lowerer allowlist change.** Placing the linkme input sections inside
   `.rodata` (with self-defined encapsulation symbols) folds the dispatch table
   into `.rodata`. The existing lowerer `.rodata` → initial-memory copy
   materializes it address-correctly; the lowerer needs no new section in its
   PROGBITS allowlist.

## Components

All paths are in `sdk_rs`. No new crate is required: the registry and `_start`
live in `groundhog-rv32-runtime` because they are inseparable from its linker
script (the encapsulation-symbol names track the slice name).

### 1. Entrypoint registry — `crates/rv32-runtime`

`groundhog-rv32-runtime` takes a new `linkme` dependency (build-relevant only on
rv32; the crate already `compile_error!`s on other targets).

```rust
#[repr(C)]
pub struct Entrypoint {
    pub method_id: u64,
    pub wrapper: extern "C" fn(),
}

#[linkme::distributed_slice]
pub static GROUNDHOG_RV32_ENTRYPOINTS: [Entrypoint];
```

`wrapper` is the existing per-method `extern "C"` wrapper. Note that wrapper
diverges (its body ends in `contract_return_encoded`, a `-> !` Return syscall),
so it never actually returns to the dispatcher.

### 2. Dispatcher — `crates/rv32-runtime`

```rust
/// Pure lookup, host-testable.
pub fn lookup(table: &[Entrypoint], method_id: u64) -> Option<extern "C" fn()> {
    table.iter().find(|e| e.method_id == method_id).map(|e| e.wrapper)
}

extern "C" fn rv32_dispatch(method_id: u64) -> ! {
    if let Some(wrapper) = lookup(&GROUNDHOG_RV32_ENTRYPOINTS, method_id) {
        wrapper();
    }
    // Reached only if (a) method_id is unknown, or (b) a wrapper returns
    // (it shouldn't). Deterministic trap — the Decision-2 default arm /
    // host-filtered backstop. Same code regardless of unknown-method policy.
    deterministic_trap()
}
```

`lookup` does a linear scan, matching v3's behavior and the runtime doc's
acknowledgement that a linear `match` is `O(methods)`. First match wins on the
(astronomically unlikely) cross-interface `method_id` collision — same risk
profile as v3 symbol collisions; not newly introduced and not solved here.

### 3. `_start` naked shim — `crates/rv32-runtime/src/start.rs`

Replaces today's trapping stub:

```rust
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    // Set sp; preserve a0/a1; tail-call the dispatcher.
    // `naked` guarantees no prologue touches a0/a1.
    core::arch::naked_asm!(
        "la sp, __stack_top",
        "andi sp, sp, -16",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
}
```

`_start` sits at PC 0 (linker `ENTRY(_start)` + `e_entry == 0`, already
enforced). The runtime enters here with `a0`/`a1` seeded; the tail-call forwards
them as the `u64` argument.

### 4. Macro change — `crates/macros/src/common/exports.rs::generate_method`

Per method, replace the rv32 export attributes with a registration:

```rust
// Keep for WASM (host dispatch):
#[cfg_attr(target_family = "wasm", unsafe(export_name = #export_name))]
pub extern "C" fn #extern_name() { #internal_name() }

// rv32 (guest dispatch): self-register into the distributed slice.
#[cfg(all(target_arch = "riscv32", feature = "entrypoints"))]
#[::sdk::__rv32_linkme::distributed_slice(::sdk::__rv32::GROUNDHOG_RV32_ENTRYPOINTS)]
#[linkme(crate = ::sdk::__rv32_linkme)]
static _GH_RV32_ENTRY: ::sdk::__rv32::Entrypoint =
    ::sdk::__rv32::Entrypoint { method_id: #method_id_u64, wrapper: #extern_name };
```

- **Drop** the rv32 arm of `export_name` and the `.text.gh_*` `link_section`
  entirely (lines ~430–442). Keep them for `target_family = "wasm"`.
- `#extern_name` / `#internal_name` bodies are unchanged — only how the wrapper
  is discovered changes.
- `method_id_u64` is `u64::from_be_bytes(entrypoint)` — the existing selector
  (the same value `#[export_name]` encodes as hex and `Entrypoints::into_u64()`
  exposes).
- Each `#[contract] impl` (including multiple trait impls on one type) emits its
  own registrations; the slice collects them all.

### 5. Linker script — `crates/rv32-runtime/link/groundhog-rv32.ld`

- Remove `KEEP(*(.text.gh_*))` (dead — no longer emitted). Keep
  `KEEP(*(.text._start))`.
- Inside `.rodata`, place the linkme sections with self-defined encapsulation
  symbols so the table rides the `.rodata` copy into the initial-memory image:

```ld
  .rodata : ALIGN(8)
  {
    . = ALIGN(8);
    __start_linkme_GROUNDHOG_RV32_ENTRYPOINTS = .;
    KEEP(*(linkme_GROUNDHOG_RV32_ENTRYPOINTS))
    __stop_linkme_GROUNDHOG_RV32_ENTRYPOINTS = .;
    __start_linkm2_GROUNDHOG_RV32_ENTRYPOINTS = .;
    KEEP(*(linkm2_GROUNDHOG_RV32_ENTRYPOINTS))
    __stop_linkm2_GROUNDHOG_RV32_ENTRYPOINTS = .;
    *(.srodata .srodata.*)
    *(.rodata .rodata.*)
  } > RAM
```

(The exact `linkme_…` / `linkm2_…` section names are produced by linkme from the
slice name `GROUNDHOG_RV32_ENTRYPOINTS`; the spike used them verbatim and linked
clean.)

### 6. Re-export plumbing — `crates/sdk`

The macro emits `::sdk::__rv32::…` and `::sdk::__rv32_linkme`. Provide them on
rv32 (independent of the `mock` feature that currently gates `__linkme`):

```rust
#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub extern crate linkme as __rv32_linkme;

#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub mod __rv32 {
    pub use groundhog_rv32_runtime::{Entrypoint, GROUNDHOG_RV32_ENTRYPOINTS};
}
```

`sdk` already depends on `groundhog-rv32-runtime` on rv32, so this is a
re-export, not a new dependency. `linkme` becomes a non-optional `sdk` dep on the
rv32 target.

## Data flow

```
hogd: enter PC 0 with a0 = method_id_lo, a1 = method_id_hi
  → _start (naked): set sp, preserve a0/a1, tail rv32_dispatch
    → rv32_dispatch(method_id: u64): lookup(GROUNDHOG_RV32_ENTRYPOINTS, id)
      → match: wrapper()  → get_calldata → decode args → invoke method
                          → contract_return_encoded (Return syscall, diverges)
      → no match: deterministic_trap()  (Decision-2 backstop / miss)
```

## Unknown-method handling (Decision 2)

The runtime-side doc leaves the unknown-method policy (metered guest miss vs
host-filtered zero-gas miss) open. **The SDK does not fork on it:** the `_start`
default arm is a deterministic trap either way — the real miss path under Option
A, an unreachable host-filtered backstop under Option B. The method-id set hogd
needs under Option B already exists in the `groundhogspecv0` spec sidecar the
macro emits (the entrypoint IDs); **no new SDK surface is needed** for any
Decision-2 outcome. This work is therefore not blocked on Decision 2.

## Strip & artifact size (avoid the misread)

- The v4 lowerer no longer scans `.symtab`, so it **stops mandating unstripped
  input** (`build_risc.sh` currently forces `CARGO_PROFILE_RELEASE_STRIP=false`
  precisely because v3 needs the entrypoint symbols). Dropping that mandate
  *enables* stripping; it does not disable it.
- Stripping changes only the **intermediate ELF**, not the lowered `.rv32a`: the
  artifact body is `header || code || initial_memory` and never packages
  `.symtab`/debug. Strip on/off → identical `.rv32a`.
- The dispatch change is size-neutral-to-better: the per-method PC table that
  lived in the **artifact header** is gone; the replacement is a tiny `.rodata`
  table (~16 bytes/method; 48 bytes for 3 methods in the spike) inside the
  initial-memory image.
- The real `.rv32a` size win is **Decision 5** (stop zero-padding the
  initial-memory image to 64 KiB), which is independent of dispatch and handled
  on the lowerer/runtime side.

## Testing

1. **Host unit test** of `lookup(&[Entrypoint], id)`: hit, miss, first-match-wins.
2. **Macro token tests** (`crates/macros`): assert rv32 generation emits the
   `distributed_slice` registration and **no** `export_name` / `.text.gh_*`;
   assert WASM still emits `export_name`. Invert the existing
   `free_function_exports_include_rv32_attributes` test
   (`crates/macros/src/common/exports.rs`), which currently asserts the rv32
   attrs are present.
3. **Build-level assertion** (script or xtask) against a real rv32 contract
   build:
   - no `16-HEX` / `.text.gh_*` symbols remain;
   - `.rodata` carries the entrypoint table with the expected `method_id`s;
   - `_start` is at PC 0;
   - a stripped release build still emits valid `.spec.json` / `.spec.xdr`
     (Decision 4: spec ALLOC sections survive strip).
4. **Smoke** (runtime-side, listed for completeness): call a method and an
   unknown method-id; assert the decided outcomes. Lands with the hogd cut.

## Cross-repo contract

This SDK work assumes — and must land together with — the following (NOT done in
`sdk_rs`):

- **hogd:** enter at PC 0; seed `a0` = `method_id & 0xFFFF_FFFF`,
  `a1 = method_id >> 32`, after `reset(gas)` + initial-memory staging and before
  `run()`; on both `Machine` and `ThreadedMachine`.
- **lowerer (`groundhog-rpc`):** stop requiring/scanning `.text.gh_*` /
  `16-HEX` symbols; validate the entry via `e_entry == 0` + PT_LOAD; stop
  building the artifact dispatch PC table; drop the forced
  `CARGO_PROFILE_RELEASE_STRIP=false`. **Keep the existing `.rodata` copy
  unchanged — no allowlist entry is needed** (the spike confirmed the table
  folds into `.rodata`).

Because removing `.text.gh_*` breaks the v3 lowerer, this is a coordinated
atomic cut (the chosen "clean replacement now" sequencing). `integration_test`
CI (the `contracts` submodule) needs a matching hogd/lowerer before it goes
green again.

## Out of scope

Artifact v4 header/format, the v4 fingerprint re-pin, Decision 5 (initial-memory
trimming), and the unknown-method policy decision — all runtime/lowerer-side.
