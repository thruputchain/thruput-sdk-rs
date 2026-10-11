# Auto-bundled guest runtimes: extract wasm runtime + bundle via SDK

**Date:** 2026-05-31
**Status:** Approved (design); pending implementation plan

## Problem

A wasm contract is a single `cdylib` crate: its `lib.rs` is the deployable
artifact, and the `#[panic_handler]` + `#[global_allocator]` it needs come from
`sdk` (an rlib dependency) automatically. No extra crate, no boilerplate.

A rv32 contract is not symmetric. The bare-metal runtime (`_start`, panic
handler, allocator, linker script) lives in a separate `groundhog-rv32-runtime`
crate, and each contract needs a thin `*-rv32` bin crate whose `main.rs` must
explicitly `extern crate groundhog_rv32_runtime;` to force those items to link:

```rust
#![no_std]
#![no_main]
extern crate groundhog_rv32_runtime;   // <-- the boilerplate we want to remove
extern crate counter;
```

We want building a rv32 target to feel like building a wasm target: the author
should not have to name the runtime crate. Concretely:

1. Extract the wasm runtime (panic handler + allocator) out of `sdk` into its
   own `groundhog-wasm-runtime` crate, mirroring `groundhog-rv32-runtime`.
2. Make `sdk` automatically pull in the correct runtime crate for the target,
   so neither the contract nor the rv32 bin needs to declare it.

## Background (verified facts)

- The rv32 runtime is fully standalone: bump allocator, `ebreak` panic handler,
  `_start`, and a linker script published via `build.rs`. No `sdk` dependency.
- The wasm runtime currently lives *inside* `sdk`:
  - `sdk/src/allocator.rs` — `SlabAllocator` + `#[global_allocator]`, gated
    `cfg(all(target_family = "wasm", feature = "alloc"))`. Standalone (only
    needs the linker-provided `__heap_base`).
  - `sdk/src/panic.rs` — `#[panic_handler] handle_panic`, gated
    `cfg(target_arch = "wasm32")`. The bare path calls `crate::trap()`
    (`core::arch::wasm32::unreachable()`). The optional `panic_logging` path
    calls `sdk::log()` → `sys::raw_ConsoleLog` (a `groundhog-xdr` syscall).
  - The `Panic` *type* (with_message / without_message, `ReadXDR`/`WriteXDR`)
    in `panic.rs` is **not** part of the panic handler. It is used across
    `exn/*`, `throw`, `re_throw`, etc. under the `debugging` feature. It stays
    in `sdk`.
- No code outside `sdk` references `sdk::allocator` (grep-verified), so moving
  the allocator out is not a breaking change to the public surface.

### Mechanism validation (empirical)

Two assumptions were tested with throwaway crates before approving this design:

1. **The linker does not require `_start`.** Linking a rv32 object with no
   `_start` and a script with no `ENTRY()` succeeds with only a warning
   (`cannot find entry symbol _start; not setting start address`), producing a
   valid ELF with `e_entry = 0x0`. `--gc-sections` does not drop method symbols
   — they survive via `KEEP(*(.text.gh_*))`. (Relevant to a future, separate
   effort to drop `_start`; not part of this change.)

2. **Force-linking through an intermediate rlib works.** A 3-crate test
   (`rt` providing `#[panic_handler]`/`#[global_allocator]`/`_start` ← `mid`
   rlib containing only `extern crate rt as _;` ← `bin` depending solely on
   `mid`) built for `riscv32im-unknown-none-elf`:
   - Build succeeded; `_start` present in the ELF; `e_entry = 0x0`.
   - Negative control (remove the `extern crate rt as _;` from `mid`):
     `error: #[panic_handler] function required, but not found`.

   This confirms a force-link buried in `sdk` pulls the runtime's lang items and
   `_start` into a downstream bin that never names the runtime.

## Design

### 1. New crate: `crates/wasm-runtime` (`groundhog-wasm-runtime`)

Mirror of `groundhog-rv32-runtime`.

- `#![no_std]`, `crate-type = ["rlib"]`.
- Empty on non-wasm targets (so it is safe to depend on unconditionally, exactly
  like rv32-runtime is on non-rv32 targets).
- `#[cfg(target_arch = "wasm32")]`:
  - `mod allocator` — `SlabAllocator` + `#[global_allocator]`, moved verbatim
    from `sdk/src/allocator.rs` (relies on the linker-provided `__heap_base`).
  - `mod panic` — `#[panic_handler] handle_panic` + the `panic_logging`
    formatter, moved from `sdk/src/panic.rs`. `trap()` becomes a local
    `core::arch::wasm32::unreachable()`. The `panic_logging` path calls the raw
    `ConsoleLog` syscall from `groundhog-xdr` directly (best-effort; errors
    ignored, since it runs inside a panic).
- Dependencies: `groundhog-xdr` (with the `syscalls` feature) only. **Never
  depends on `sdk`** — this avoids a dependency cycle, since `sdk` will depend
  on this crate.
- Feature `panic_logging` (forwarded from `sdk`).

### 2. `sdk` changes

- Remove `mod allocator;` / `pub mod allocator` and the `#[panic_handler]` from
  `sdk/src/panic.rs`. **Keep the `Panic` type** and its `with_message` /
  `without_message` modules — they are independent of the handler and used by
  the exception machinery.
- Add target-gated dependencies in `sdk/Cargo.toml`:
  ```toml
  [target.'cfg(target_arch = "riscv32")'.dependencies]
  groundhog-rv32-runtime = { workspace = true }

  [target.'cfg(target_family = "wasm")'.dependencies]
  groundhog-wasm-runtime = { workspace = true }
  ```
- Force-link in `sdk/src/lib.rs`:
  ```rust
  #[cfg(target_arch = "riscv32")]
  extern crate groundhog_rv32_runtime as _;

  #[cfg(target_family = "wasm")]
  extern crate groundhog_wasm_runtime as _;
  ```
- Forward the panic-logging feature: `sdk`'s `panic_logging` feature enables
  `groundhog-wasm-runtime/panic_logging`.

### 3. `counter-rv32/src/main.rs`

Drop `extern crate groundhog_rv32_runtime;`. The file becomes:

```rust
#![no_std]
#![no_main]
extern crate counter;
```

`#![no_main]` and `extern crate counter;` remain irreducible here: a bare-metal
executable needs a crate root carrying `#![no_main]`, and `extern crate counter`
forces the contract's macro-emitted method symbols to link. Removing those is a
separate effort (lowerer-side `_start` removal + a build-time wrapper generator)
and is out of scope.

### 4. Linker-script search path (verified)

The rv32 linker-script *search path* is published by `rv32-runtime`'s
`build.rs` via `cargo:rustc-link-search`. Before this change the bin depended on
`rv32-runtime` directly; after it, the dependency is only transitive
(bin → counter → sdk → rv32-runtime).

**Verified:** `cargo:rustc-link-search` propagates transitively to the final
bin link. `counter-rv32` builds for `riscv32im-unknown-none-elf` with **no**
direct `groundhog-rv32-runtime` dependency in its `Cargo.toml` — `-Tgroundhog-rv32.ld`
resolves and the ELF contains `_start` (at vaddr 0) plus the four method entry
symbols. The direct dependency was therefore removed from `counter-rv32`. No
fallback needed.

(Note: `-Tgroundhog-rv32.ld` itself is supplied via rustflags in
`example-contracts/.cargo/config.toml` and is unaffected by this change.)

## Testing

- **rv32 link:** `counter-rv32` builds for `riscv32im-unknown-none-elf` with the
  slimmer `main.rs`; the ELF still contains `_start` and the `.text.gh_*` method
  entry symbols the lowerer needs.
- **wasm link:** a wasm contract builds with exactly one `#[global_allocator]`
  and one `#[panic_handler]`, both sourced from `groundhog-wasm-runtime`; no
  duplicate-symbol errors.
- **native/mock:** `cargo test` with the `dev`/`mock` features is unaffected —
  both runtime crates compile empty off-target and are not force-linked (the
  `extern crate … as _;` lines are `cfg`-gated).
- **panic_logging:** still logs on wasm via the `groundhog-xdr` `ConsoleLog`
  syscall when the feature is enabled.

## Implementation notes (deltas from the original design)

- **Target gating is a `compile_error!`, not silent emptiness.** Each runtime
  crate has `#[cfg(not(any(target_arch = "<t>", doc)))] compile_error!(...)` at
  the crate root and leaves its modules ungated, so building it for the wrong
  target fails loudly instead of producing an empty lib. `doc` is allowed so
  rustdoc/docs.rs can still render the items (wasm needs a doc-only `trap()`
  stub since `core::arch::wasm32` is absent off-wasm).
- **Excluded from `default-members`.** Because CI's host-wide `cargo
  build/test/doc` build every default member, the `compile_error!` would fire
  on the host. The runtime crates stay `members` (field inheritance, `-p`
  builds, publishing via sdk) but are removed from `default-members`, so
  host-wide commands skip them while `-p ... --target ...` and the transitive
  sdk dependency still build them.
- **`alloc` feature.** Both runtimes gate the `#[global_allocator]` behind an
  `alloc` feature, forwarded from `sdk`'s `alloc` feature, so contracts that
  don't allocate don't carry an allocator.

## Out of scope

- Removing `_start` from the rv32 ELF (separate effort; requires a lowerer
  change in the `groundhog-rpc` repo to drop the `_start@0` assertion).
- A build-time generator that synthesizes the `*-rv32` bin wrapper so authors
  write only `lib.rs`. This change reduces the wrapper to irreducible lines but
  does not eliminate the wrapper crate.
- Any change to the `thruput build` CLI (lives in `groundhog-rpc`).
