# RV32 Guest-Side Dispatch (SDK) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make rv32 contracts dispatch their own methods from `_start` (reading the invoked `method_id` from `a0`/`a1`), via a `linkme` registry populated by `#[contract]` — and remove the rv32 `.text.gh_*` / `export_name` method-export symbols.

**Architecture:** Each `#[contract]` method self-registers `(method_id, wrapper)` into a `linkme::distributed_slice` (the on-guest analog of the existing native `CONTRACT_MOCK_FRAGMENTS`). A naked `_start` at PC 0 preserves `a0`/`a1` and tail-calls `rv32_dispatch(method_id: u64)`, which linear-scans the slice and calls the matching wrapper. The slice lands in `.rodata` (via linker-script placement) so the lowerer's existing `.rodata` copy materializes it — no lowerer allowlist change.

**Tech Stack:** Rust (nightly-2026-01-30), `riscv32im-unknown-none-elf`, `linkme` 0.3, proc-macro (`syn`/`quote`), rust-lld + custom linker script.

**Spec:** `docs/superpowers/specs/2026-06-02-rv32-guest-dispatch-sdk-design.md`. All claims below were validated by a standalone rv32 spike (linkme links stripped; `a0`/`a1`→`u64` is free via the C ABI; the table folds into `.rodata`; naked `_start` + `tail {sym}` compiles and preserves `a0`/`a1`).

**Cross-repo note (do NOT do here):** hogd must enter PC 0 and seed `a0 = method_id & 0xFFFF_FFFF`, `a1 = method_id >> 32`; the lowerer must stop scanning `.text.gh_*` and stop building the dispatch PC table. Removing `.text.gh_*` here breaks the v3 lowerer — this is the agreed atomic cut. `integration_test` CI (the `contracts` submodule) stays red until a matching hogd/lowerer lands.

**Plan-doc convention:** This file lives under `docs/superpowers/` and is a working artifact — do NOT `git add` it. The code commits in each task are real.

---

## File Structure

- `crates/rv32-runtime/src/lib.rs` — module wiring + host-test gating. Modify.
- `crates/rv32-runtime/src/dispatch.rs` — `Entrypoint` type + pure `lookup` (host-testable). **Create.**
- `crates/rv32-runtime/src/start.rs` — rv32-only: `GROUNDHOG_RV32_ENTRYPOINTS` slice, `rv32_dispatch`, naked `_start`. Modify (rewrite).
- `crates/rv32-runtime/Cargo.toml` — add `linkme` (rv32-target dep). Modify.
- `crates/rv32-runtime/link/groundhog-rv32.ld` — drop `.text.gh_*` KEEP; place linkme sections in `.rodata`. Modify.
- `crates/sdk/Cargo.toml` — `linkme` as rv32-target dep. Modify.
- `crates/sdk/src/lib.rs` — `__rv32_linkme` + `__rv32` re-exports (rv32). Modify.
- `crates/macros/src/common/exports.rs` — `generate_method`: drop rv32 export attrs, emit registration. Modify `tests` too.
- `scripts/check-rv32-dispatch.sh` — objdump assertions over a linked rv32 ELF (driven by the cross-repo smoke). **Create.**

---

## Task 1: Pure dispatch lookup + host-test gating

**Files:**
- Create: `crates/rv32-runtime/src/dispatch.rs`
- Modify: `crates/rv32-runtime/src/lib.rs`

- [ ] **Step 1: Create `dispatch.rs` with the type, lookup, and a failing-on-purpose test**

Create `crates/rv32-runtime/src/dispatch.rs`:

```rust
//! Guest dispatch primitives, shared by the rv32 `_start` dispatcher.
//!
//! `Entrypoint` is the registry element each `#[contract]` method emits via a
//! `linkme::distributed_slice`. `lookup` is the pure scan used by the
//! dispatcher; it is target-independent so it can be unit-tested on the host.

/// One method's dispatch record: its selector and the wrapper to invoke.
///
/// `wrapper` runs the calldata-decode / invoke / `contract_return_encoded`
/// path; it diverges (never returns to the dispatcher) in practice.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Entrypoint {
    pub method_id: u64,
    pub wrapper: extern "C" fn(),
}

/// Return the wrapper registered for `method_id`, or `None` if unknown.
/// Linear scan; first match wins on a (vanishingly unlikely) id collision.
pub fn lookup(
    table: &[Entrypoint],
    method_id: u64,
) -> Option<extern "C" fn()> {
    table
        .iter()
        .find(|e| e.method_id == method_id)
        .map(|e| e.wrapper)
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn a() {}
    extern "C" fn b() {}
    extern "C" fn c() {}

    fn table() -> [Entrypoint; 3] {
        [
            Entrypoint { method_id: 0x1111_2222_3333_4444, wrapper: a },
            Entrypoint { method_id: 0x5555_6666_7777_8888, wrapper: b },
            // duplicate id to prove "first match wins"
            Entrypoint { method_id: 0x5555_6666_7777_8888, wrapper: c },
        ]
    }

    #[test]
    fn finds_known_method() {
        let t = table();
        let got = lookup(&t, 0x1111_2222_3333_4444).unwrap();
        assert_eq!(got as usize, a as usize);
    }

    #[test]
    fn first_match_wins_on_duplicate() {
        let t = table();
        let got = lookup(&t, 0x5555_6666_7777_8888).unwrap();
        assert_eq!(got as usize, b as usize);
    }

    #[test]
    fn unknown_method_is_none() {
        let t = table();
        assert!(lookup(&t, 0xDEAD_BEEF_DEAD_BEEF).is_none());
    }
}
```

- [ ] **Step 2: Wire the module + host-test gating into `lib.rs`**

In `crates/rv32-runtime/src/lib.rs`:

1. Change the first line from `#![no_std]` to:
```rust
#![cfg_attr(not(test), no_std)]
```

2. Change the `compile_error!` cfg to also allow `test`:
```rust
#[cfg(not(any(target_arch = "riscv32", doc, test)))]
compile_error!(
    "groundhog-rv32-runtime can only be built for the riscv32 target"
);
```

3. Replace the module declarations block (currently the `allocator`/`panic`/`start` lines and `pub use start::_start;`) with:
```rust
// Pure, target-independent — compiled everywhere (host tests included).
mod dispatch;
pub use dispatch::{Entrypoint, lookup};

// Allocator gated on `alloc`, matching `sdk`'s `alloc` feature. rv32-only:
// it binds linker-script externs and installs a `#[global_allocator]`.
#[cfg(all(feature = "alloc", target_arch = "riscv32"))]
mod allocator;

// rv32-only: these define `#[panic_handler]`, naked asm, and the linkme slice,
// none of which can compile on the host.
#[cfg(target_arch = "riscv32")]
mod panic;

#[cfg(target_arch = "riscv32")]
mod start;

#[cfg(target_arch = "riscv32")]
pub use start::{GROUNDHOG_RV32_ENTRYPOINTS, _start};
```

(Note: `GROUNDHOG_RV32_ENTRYPOINTS` is added to `start.rs` in Task 2; this `pub use` will not resolve until then. That is expected — Task 1's verification is the host test, which does not touch `start.rs`. If you want Task 1 to compile the rv32 target in isolation, temporarily drop `GROUNDHOG_RV32_ENTRYPOINTS` from the `pub use` and add it in Task 2.)

- [ ] **Step 3: Run the host test, expect PASS**

Run: `cargo test -p groundhog-rv32-runtime --lib dispatch`
Expected: 3 tests pass (`finds_known_method`, `first_match_wins_on_duplicate`, `unknown_method_is_none`).

If you see `error: could not compile … compile_error!`, the cfg in Step 2.2 was not applied. If you see `no_std`/`#[panic_handler]` conflicts, a rv32-only module (`panic`/`start`/`allocator`) is not gated by `target_arch = "riscv32"`.

- [ ] **Step 4: Commit**

```bash
git add crates/rv32-runtime/src/dispatch.rs crates/rv32-runtime/src/lib.rs
git commit -m "rv32-runtime: add pure Entrypoint/lookup dispatch primitive with host tests"
```

---

## Task 2: linkme dependency + registry slice + dispatcher

**Files:**
- Modify: `crates/rv32-runtime/Cargo.toml`
- Modify: `crates/rv32-runtime/src/start.rs`

- [ ] **Step 1: Add `linkme` as an rv32-target dependency**

In `crates/rv32-runtime/Cargo.toml`, under the existing `[dependencies]` section (which is currently empty/comment-only), add a target-gated block:

```toml
# Guest dispatch registry: each #[contract] method self-registers into a
# distributed_slice the `_start` dispatcher scans. This exact pin
# (default features) was spike-verified to link on bare-metal rv32 with
# --gc-sections + strip + LTO. Do not trim features without re-spiking.
[target.'cfg(target_arch = "riscv32")'.dependencies]
linkme = "0.3"
```

- [ ] **Step 2: Rewrite `start.rs` — slice + dispatcher + naked `_start`**

Replace the entire contents of `crates/rv32-runtime/src/start.rs` with:

```rust
//! rv32 `_start`: the guest dispatch entry.
//!
//! The hogd VM enters every call at PC 0 (`_start`) with the invoked
//! `method_id` split across `a0` (low 32) and `a1` (high 32). `_start` is a
//! naked shim: it sets `sp` and tail-calls `rv32_dispatch` WITHOUT touching
//! `a0`/`a1`, so the RISC-V C ABI reassembles them into the `u64` argument for
//! free. The dispatcher scans the linkme registry populated by the
//! `#[contract]` macro and calls the matching method wrapper.

use crate::dispatch::{Entrypoint, lookup};
use core::arch::{asm, naked_asm};
use linkme::distributed_slice;

/// Registry of every contract method, populated at link time by the
/// `#[contract]` macro (one `#[distributed_slice]` static per method).
#[distributed_slice]
pub static GROUNDHOG_RV32_ENTRYPOINTS: [Entrypoint];

/// Deterministic, gas-free-ish trap. Reached on an unknown method id (the
/// Decision-2 default arm / host-filtered backstop) or if a wrapper returns
/// (it shouldn't — wrappers diverge via the Return syscall). hogd traps
/// `ebreak` as `Trap::kBreakpoint` → deterministic failure.
fn deterministic_trap() -> ! {
    unsafe { asm!("ebreak", options(noreturn)) }
}

/// C-ABI entry called by the naked `_start`. `method_id` arrives reassembled
/// from the seeded `a0`/`a1`.
extern "C" fn rv32_dispatch(method_id: u64) -> ! {
    if let Some(wrapper) = lookup(&GROUNDHOG_RV32_ENTRYPOINTS, method_id) {
        wrapper();
    }
    deterministic_trap()
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    // Set sp to the top of guest memory (16-byte aligned) and tail-call the
    // dispatcher. `naked` guarantees no prologue clobbers a0/a1.
    naked_asm!(
        "la sp, __stack_top",
        "andi sp, sp, -16",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
}
```

- [ ] **Step 3: Compile-check for the rv32 target (rlib; no link step needed)**

Run: `cargo build -p groundhog-rv32-runtime --target riscv32im-unknown-none-elf`
Expected: builds clean. (This compiles the rlib; the linker script is only exercised when a downstream bin links.)

If `naked_asm!` is unresolved, confirm the `use core::arch::{asm, naked_asm};` import. If `tail` errors, it is a valid pseudo-instruction for rv32 — confirm the `{dispatch}` / `dispatch = sym rv32_dispatch` operand spelling.

- [ ] **Step 4: Re-run the host test to confirm no host regression**

Run: `cargo test -p groundhog-rv32-runtime --lib dispatch`
Expected: 3 tests pass (host build excludes `start.rs`).

- [ ] **Step 5: Commit**

```bash
git add crates/rv32-runtime/Cargo.toml crates/rv32-runtime/src/start.rs
git commit -m "rv32-runtime: add linkme registry slice, rv32_dispatch, and naked _start"
```

---

## Task 3: Linker script — drop `.text.gh_*`, place registry in `.rodata`

**Files:**
- Modify: `crates/rv32-runtime/link/groundhog-rv32.ld`

- [ ] **Step 1: Remove the `.text.gh_*` KEEP**

In `crates/rv32-runtime/link/groundhog-rv32.ld`, inside the `.text` output section, delete the block:

```ld
    /* Method entry points emitted by the #[contract] macro on rv32.
     * The macro places each method in `.text.gh_<EXPORT_NAME>` where
     * EXPORT_NAME is the 16-char hex ContractEntrypointID also used
     * for the wasm export name. The lowerer reads this symbol table
     * to build the artifact dispatch table.
     */
    KEEP(*(.text.gh_*))
```

Keep `KEEP(*(.text._start))` and the `*(.text .text.*)` line.

- [ ] **Step 2: Place the linkme registry sections inside `.rodata`**

Replace the `.rodata` output section so the linkme sections are bracketed by explicit encapsulation symbols and copied as part of `.rodata`:

```ld
  . = ALIGN(8);
  .rodata : ALIGN(8)
  {
    /* Guest dispatch registry (linkme distributed_slice). Placed inside
     * .rodata with explicit __start_/__stop_ encapsulation symbols so the
     * table rides the lowerer's existing .rodata -> initial-memory copy,
     * instead of an orphan section the PROGBITS allowlist would skip.
     * Section names are produced by linkme from the slice name
     * GROUNDHOG_RV32_ENTRYPOINTS. */
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

Also update the `.text` section header comment that references the dispatch table (the comment above `KEEP(*(.text._start))`) to drop the "artifact dispatch routes method ID 0 here" framing, since `_start` is now the sole entry:

```ld
    /* _start is the linker entry and the sole dispatch surface: the VM
     * always enters here at PC 0 with the method id in a0/a1. KEEP it so
     * dead-code elimination can't drop it. */
    KEEP(*(.text._start))
```

- [ ] **Step 3: Verify (deferred to Task 7's linked-binary check)**

The linker script is only exercised when a contract bin links. There is no in-repo bin to link here (see Task 7). Re-confirm the rlib still builds:

Run: `cargo build -p groundhog-rv32-runtime --target riscv32im-unknown-none-elf`
Expected: builds clean (rlib build does not consume the linker script, but confirms no syntax drift was introduced elsewhere).

- [ ] **Step 4: Commit**

```bash
git add crates/rv32-runtime/link/groundhog-rv32.ld
git commit -m "rv32-runtime: linker script — drop .text.gh_*, fold dispatch registry into .rodata"
```

---

## Task 4: SDK re-exports + linkme dependency

**Files:**
- Modify: `crates/sdk/Cargo.toml`
- Modify: `crates/sdk/src/lib.rs`

- [ ] **Step 1: Add `linkme` as an rv32-target dependency of `sdk`**

In `crates/sdk/Cargo.toml`, find the existing rv32 target dependency block:

```toml
[target.'cfg(target_arch = "riscv32")'.dependencies]
groundhog-rv32-runtime = { workspace = true }
```

Add `linkme` to it (the macro's generated registration needs `::sdk::__rv32_linkme`):

```toml
[target.'cfg(target_arch = "riscv32")'.dependencies]
groundhog-rv32-runtime = { workspace = true }
linkme = { workspace = true }
```

(`linkme` is already a workspace dependency, currently `optional` for the `mock` feature; this target block makes it non-optional on rv32.)

- [ ] **Step 2: Add the re-exports in `lib.rs`**

In `crates/sdk/src/lib.rs`, right after the rv32 force-link line (`#[cfg(target_arch = "riscv32")] extern crate groundhog_rv32_runtime as _;`), add:

```rust
// Guest-dispatch plumbing for the `#[contract]` macro on rv32. The macro emits
// `#[::sdk::__rv32_linkme::distributed_slice(::sdk::__rv32::GROUNDHOG_RV32_ENTRYPOINTS)]`
// registrations; these re-exports give that generated code a stable path.
#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub extern crate linkme as __rv32_linkme;

#[cfg(target_arch = "riscv32")]
#[doc(hidden)]
pub mod __rv32 {
    pub use groundhog_rv32_runtime::{Entrypoint, GROUNDHOG_RV32_ENTRYPOINTS};
}
```

- [ ] **Step 3: Compile-check `sdk` for the rv32 target**

Run: `cargo build -p sdk --target riscv32im-unknown-none-elf --features storage`
Expected: builds clean.

If `groundhog_rv32_runtime::Entrypoint` / `GROUNDHOG_RV32_ENTRYPOINTS` are unresolved, confirm Tasks 1–2 exported them from `groundhog-rv32-runtime`.

- [ ] **Step 4: Confirm host build of `sdk` is unaffected**

Run: `cargo build -p sdk`
Expected: builds clean (the new items are `cfg(target_arch = "riscv32")`, absent on host).

- [ ] **Step 5: Commit**

```bash
git add crates/sdk/Cargo.toml crates/sdk/src/lib.rs
git commit -m "sdk: re-export rv32 dispatch registry + linkme for the contract macro"
```

---

## Task 5: Macro — drop rv32 export symbols, emit registry registration

**Files:**
- Modify: `crates/macros/src/common/exports.rs` (`generate_method` ~lines 423–453, and the `tests` module ~lines 541–557)

- [ ] **Step 1: Update the failing macro token test first (TDD)**

In `crates/macros/src/common/exports.rs`, replace the existing `free_function_exports_include_rv32_attributes` test with one asserting the new shape:

```rust
    #[test]
    fn free_function_exports_emit_rv32_registry_not_export_symbols() {
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        // WASM still uses host-side export dispatch.
        assert!(generated.contains("target_family = \"wasm\""));
        assert!(generated.contains("export_name"));

        // rv32 now self-registers into the distributed slice...
        assert!(generated.contains("target_arch = \"riscv32\""));
        assert!(generated.contains("GROUNDHOG_RV32_ENTRYPOINTS"));
        assert!(generated.contains("distributed_slice"));

        // ...and the old rv32 dispatch symbols are gone.
        assert!(!generated.contains(".text.gh_"));
        assert!(!generated.contains("link_section"));
    }
```

- [ ] **Step 2: Run it, expect FAIL**

Run: `cargo test -p groundhog-macros --lib free_function_exports_emit_rv32_registry_not_export_symbols`
Expected: FAIL — current codegen still emits `link_section` / `.text.gh_` and no `GROUNDHOG_RV32_ENTRYPOINTS`.

- [ ] **Step 3: Rewrite the codegen in `generate_method`**

In `generate_method`, delete the `rv32_link_section` line:

```rust
    let rv32_link_section = format!(".text.gh_{}", export_name);
```

and add (just before the final `Ok(quote! { … })`) the selector literal and a unique static name:

```rust
    let method_id_u64 = u64::from_be_bytes(*entrypoint);
    let method_id_lit = proc_macro2::Literal::u64_suffixed(method_id_u64);
    let rv32_entry_static = format_ident!("__GH_RV32_ENTRY_{}", ident);
```

Then replace the final returned `Ok(quote! { … })` block with:

```rust
    Ok(quote! {

        #[doc=#export_docstr]
        #[cfg(feature = "entrypoints")]
        #[cfg_attr(
            target_family = "wasm",
            unsafe(export_name = #export_name)
        )]
        pub extern "C" fn #extern_name() {
            #internal_name()
        }

        // rv32 guest dispatch: register this method into the runtime's
        // distributed_slice instead of exporting a per-method symbol.
        #[cfg(all(feature = "entrypoints", target_arch = "riscv32"))]
        #[::sdk::__rv32_linkme::distributed_slice(
            ::sdk::__rv32::GROUNDHOG_RV32_ENTRYPOINTS
        )]
        #[linkme(crate = ::sdk::__rv32_linkme)]
        #[allow(non_upper_case_globals)]
        static #rv32_entry_static: ::sdk::__rv32::Entrypoint =
            ::sdk::__rv32::Entrypoint {
                method_id: #method_id_lit,
                wrapper: #extern_name,
            };

        #inline
        #[allow(non_snake_case)]
        pub fn #internal_name() {
            #wrapped_call
        }
    })
```

- [ ] **Step 4: Run the macro test, expect PASS**

Run: `cargo test -p groundhog-macros --lib free_function_exports_emit_rv32_registry_not_export_symbols`
Expected: PASS.

- [ ] **Step 5: Sweep for other tests/asserts that pin the old rv32 symbols**

Run: `grep -rn "\.text\.gh_\|text.gh" crates/ docs/ --include=*.rs`
Expected: no remaining production references. If other macro tests assert rv32 `link_section`/`.text.gh_`, update them to the registry shape (same assertions as Step 1). Re-run `cargo test -p groundhog-macros` and confirm green.

- [ ] **Step 6: Commit**

```bash
git add crates/macros/src/common/exports.rs
git commit -m "macros: rv32 methods self-register into dispatch slice; drop .text.gh_*/export_name"
```

---

## Task 6: On-target integration build + objdump assertion script

**Files:**
- Create: `scripts/check-rv32-dispatch.sh`

- [ ] **Step 1: Confirm `sdk` + runtime build for rv32 across the published feature set**

Mirror CI's rv32 build (see `.github/workflows/publish.yml`: `--feature-powerset --skip std --skip mock --skip dev` over `groundhog-rv32-runtime sdk`). At minimum run:

```bash
cargo build -p groundhog-rv32-runtime --target riscv32im-unknown-none-elf
cargo build -p groundhog-rv32-runtime --target riscv32im-unknown-none-elf --features alloc
cargo build -p sdk --target riscv32im-unknown-none-elf
cargo build -p sdk --target riscv32im-unknown-none-elf --features storage
```

Expected: all build clean.

- [ ] **Step 2: Create the linked-binary assertion script**

This is the build-level check the cross-repo smoke runs against a *linked* rv32 contract ELF (it cannot run in this repo's gated CI because no contract bin links here — the `contracts` submodule owns that). Create `scripts/check-rv32-dispatch.sh`:

```bash
#!/usr/bin/env bash
# Assert a lowered/linked rv32 contract ELF uses guest-side dispatch:
#   - no legacy 16-hex / .text.gh_* method-export symbols remain
#   - _start is the entry at vaddr 0
#   - the linkme dispatch table is present inside .rodata
#
# Usage: scripts/check-rv32-dispatch.sh path/to/contract.elf
# Requires: llvm-objdump, llvm-readobj (or rust-objdump/readobj) on PATH.
set -euo pipefail

ELF="${1:?usage: check-rv32-dispatch.sh <elf>}"
OBJDUMP="${OBJDUMP:-$(command -v llvm-objdump || command -v rust-objdump)}"
READOBJ="${READOBJ:-$(command -v llvm-readobj || command -v rust-readobj)}"

fail() { echo "FAIL: $*" >&2; exit 1; }

# 1. No legacy per-method export sections.
if "$OBJDUMP" --section-headers "$ELF" | grep -qE '\.text\.gh_'; then
  fail ".text.gh_* sections still present (v3 dispatch surface not removed)"
fi

# 2. _start at entry vaddr 0.
ENTRY="$("$READOBJ" --file-headers "$ELF" | awk -F'[ ]+' '/Entry:/ {print $NF}')"
[ "$ENTRY" = "0x0" ] || fail "_start not at vaddr 0 (entry=$ENTRY)"

# 3. .rodata exists and is non-empty (it carries the dispatch table). The
#    cross-repo smoke additionally asserts specific method_ids appear in the
#    initial-memory image; that check lives with the lowerer's test harness.
if ! "$OBJDUMP" --section-headers "$ELF" | grep -qE '\.rodata'; then
  fail ".rodata missing (dispatch table would be unmaterialized)"
fi

echo "OK: $ELF uses guest-side dispatch (no .text.gh_*, _start@0, .rodata present)"
```

Make it executable:

```bash
chmod +x scripts/check-rv32-dispatch.sh
```

- [ ] **Step 3: Smoke-test the script against a locally-linked binary (optional, manual)**

If you have a checked-out hogd/`contracts` workspace that builds the smoke counter for rv32 against this local `sdk` (path override), run the script on the resulting ELF and expect `OK:`. Otherwise this step is validated by the cross-repo smoke. Do NOT block the commit on it.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-rv32-dispatch.sh
git commit -m "scripts: add rv32 guest-dispatch assertion for the cross-repo smoke"
```

---

## Task 7: Docs cleanup

**Files:**
- Modify: `crates/rv32-runtime/src/lib.rs` (the crate-level doc comment)

- [ ] **Step 1: Update the crate doc to describe guest dispatch**

In `crates/rv32-runtime/src/lib.rs`, update the crate-level `//!` docs that currently say the lowerer ingests `.text.gh_*` symbols. Replace the relevant paragraph (the "Conceptual model" / "method entry symbols … v3 artifact" text) with:

```rust
//! Conceptual model (cf. CKB `ckb-std`): contract logic lives in a
//! target-agnostic library crate. A thin per-contract bin crate depends
//! on the contract library AND on this runtime. The VM enters at `_start`
//! (PC 0) with the invoked method id in `a0`/`a1`; `_start` tail-calls
//! `rv32_dispatch`, which scans the `GROUNDHOG_RV32_ENTRYPOINTS` registry
//! (populated by the `#[contract]` macro, one entry per method) and invokes
//! the matching wrapper. No per-method export symbols are emitted on rv32.
```

- [ ] **Step 2: Build + lint the workspace**

Run: `cargo build && cargo test -p groundhog-rv32-runtime --lib dispatch && cargo test -p groundhog-macros`
Expected: host workspace builds; dispatch + macro tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/rv32-runtime/src/lib.rs
git commit -m "rv32-runtime: document guest-side dispatch model"
```

---

## Final verification (run after all tasks)

- [ ] `cargo build` — host workspace clean.
- [ ] `cargo test -p groundhog-rv32-runtime --lib dispatch` — 3 pass.
- [ ] `cargo test -p groundhog-macros` — green.
- [ ] `cargo build -p groundhog-rv32-runtime --target riscv32im-unknown-none-elf` — clean.
- [ ] `cargo build -p sdk --target riscv32im-unknown-none-elf --features storage` — clean.
- [ ] `grep -rn "\.text\.gh_" crates/ --include=*.rs` — no production references.

## What remains (other repos — tracked, not done here)

- hogd: enter PC 0; seed `a0`=low/`a1`=high after `reset()` + memory staging, before `run()`; on `Machine` and `ThreadedMachine`.
- lowerer (`groundhog-rpc`): stop scanning `.text.gh_*`; validate via `e_entry == 0` + PT_LOAD; drop the dispatch PC table; drop the forced `CARGO_PROFILE_RELEASE_STRIP=false`. Keep the `.rodata` copy as-is.
- Run `scripts/check-rv32-dispatch.sh` against the lowered smoke ELF as part of the cross-repo smoke.
