# RV32 `.text` shadow elimination — SDK linker side

Date: 2026-06-02
Status: design approved

## Problem

The Groundhog RV32 VM is modified-Harvard: code is fetched by PC from a separate
code memory (the artifact's `code` section), and loads/stores hit a separate data
memory (the artifact's `initial_memory`, zero-extended to `memory_bytes` at load).
The two are independent address spaces in the VM.

The SDK linker (`crates/rv32-runtime/link/groundhog-rv32.ld`) currently lays
everything into one 0-based region with `.text` first, then `.rodata`/`.data`
stacked *above* it:

```
MEMORY { RAM (rwx) : ORIGIN = 0, LENGTH = 64K }
.text 0 : { ... }      # vaddr [0, code_size)
.rodata : ALIGN(8) ... # vaddr just above code_size
.data   : ...
```

Because data sections get vaddrs above the code, and the stored data image is
address-indexed from offset 0, the image carries a code-sized zero hole beneath
the real data — the inert "shadow" of `.text` in data memory. Measured on the
counter smoke contract: `initial_memory` is ~12,716 B, of which ~12,504 are
leading zeros (the `.text` shadow + gap); only ~101 B are real `.rodata`/`.data`.
The code is effectively shipped twice — once as code, once as an equal-sized zero
hole in the data image.

## Goal

Stop shipping the `.text` shadow. The data image should contain (close to) only
the real initialized data, independent of `.text` size. Target: the counter
`.rv32a` data image drops from ~12.7 KB to a few hundred bytes.

## Approach: decouple the code and data address spaces (Option 3)

Exploit the Harvard split. Code addresses (PC space) and data addresses are
distinct in the VM, so data sections can legitimately start at low vaddrs that
numerically overlap the code's PC range — they never collide because they live
in different memories. Give data its own low VMA range so the data image is
compact from the start.

### Scope

This spec covers **only the SDK side** (`sdk_rs`): two files. The matching
lowerer relaxation lives in `groundhog-rpc` (`crates/rv32/lower/src/lower.rs`,
~line 140, which rejects data sections with `addr < text_end`) and is a
**separate PR**, in progress by the repo owner. Until that lands, ELFs produced
by the new linker script will not lower (the current lowerer rejects the
overlapping data vaddrs), so the end-to-end data-image shrink and the hogd
runtime-context smoke tests are verified on the lowerer PR, not here.

No artifact-format change and no hogd runtime change.

## The null-pointer wrinkle and the guard decision

If data sections started at vaddr 0, the first `static` would land at address 0,
making a correct Rust `&'static` to it a null reference — UB, because Rust
guarantees references are non-null (e.g. the `Option<&T>` null niche). That is
the **entire** correctness concern.

Decision: start the DATA region at a minimal guard of **`0x8`** (one alignment
unit; since `.rodata` is `ALIGN(8)`, anything in `(0, 8]` rounds to `0x8`). This
guarantees no `static` is placed at address 0, which is the only thing that
would break correct Rust. It costs ~8 stored leading-zero bytes.

Explicitly rejected: a larger "null page" guard (e.g. `0x1000`). A bigger guard
only makes *already-UB* null-plus-offset dereferences (`(*null).field`) trap more
visibly at runtime — it protects broken code, not correct code, and we don't care
about that. A page guard would also reintroduce ~4 KB of residual shadow, missing
the few-hundred-bytes target. (The layout-agnostic way to remove even the 8-byte
residual is an `initial_memory_addr` header field — artifact v5 — but that lives
in `groundhog-rpc` and is out of SDK scope.)

## Changes

### Change 1 — `crates/rv32-runtime/link/groundhog-rv32.ld`

Split the single `RAM` region into two address spaces mirroring the VM's Harvard
split:

```
MEMORY {
  CODE (rx) : ORIGIN = 0,   LENGTH = 64K        /* PC space: .text only */
  DATA (rw) : ORIGIN = 0x8, LENGTH = 64K - 0x8  /* data space, 0x8 null guard */
}
```

- `.text` → `CODE`; entry `_start` still at PC 0; `.text._start` KEEP unchanged.
- `.rodata`/`.srodata`/`.data`/`.sdata`/`.bss`/`.sbss` → `DATA`, starting at `0x8`.
  The linkme `GROUNDHOG_RV32_ENTRYPOINTS` encapsulation stays inside `.rodata`.
- Data space stays `[0, 64K)`, so `memory_bytes` is unchanged. Recompute heap/stack
  symbols off `DATA`:
  - `__stack_top = ORIGIN(DATA) + LENGTH(DATA)` = 64K (same value as today).
  - `__heap_base` after `.bss`; `__heap_end = __stack_top - __stack_size`.
  - Keep `ASSERT(__heap_base <= __heap_end, ...)`.
- Rewrite the header comment block: data no longer lives "above .text"; document
  that CODE and DATA VMAs intentionally overlap numerically because they are
  distinct VM memories, and explain the `0x8` guard.

Runtime symbols that must remain valid (all data-space): `__stack_top` (loaded
into `sp` by `_start` via `la sp, __stack_top`), `__heap_base`, `__heap_end`
(bound by the allocator in `crates/rv32-runtime/src/allocator.rs`).

### Change 2 — `example-contracts/.cargo/config.toml`

Add to the `[target.riscv32im-unknown-none-elf]` rustflags:

```
"-C", "link-arg=--no-check-sections",
```

CODE `[0, ~10 KB)` and DATA `[0x8, …)` overlap in VMA, so LLD's default
overlap check (`--check-sections`) would abort the link. The overlap is nominal
(separate VM address spaces), so we disable the check. Add a comment explaining
why. (Confirmed: `rust-lld -flavor gnu` supports `--no-check-sections`.)

## Verification

Achievable in `sdk_rs` now:

- Example contracts link cleanly under the new script — no LLD overlap error.
- `llvm-readelf -S` on the counter contract: `.text` at VMA 0, and
  `.rodata`/`.data` now at low VMAs (~`0x8`), no longer stacked above the code.
- All example contracts still build.
- Runtime symbols (`__stack_top`, `__heap_base`, `__heap_end`) resolve into data
  space; `_start` at PC 0.

Deferred to the lowerer PR (`groundhog-rpc`):

- The actual `.rv32a` data-image shrink from ~12.7 KB to a few hundred bytes.
- The hogd runtime-context smoke tests (`increment`/`get` round trip, the
  `LoweredSdkArtifactSmoke` get) passing unchanged — proves data pointers still
  resolve under the new layout.

## Open item to check during implementation

Whether `integration_test/contracts` (a git submodule) links any RV32 contracts
with its own cargo config; if so it needs the same `--no-check-sections` flag,
which is a submodule coordination concern outside this SDK PR's direct edits.
`integration_test/.cargo/config.toml` is wasm-only, so this is likely a no-op —
confirm during implementation.

## Out of scope

- Lowerer relaxation in `groundhog-rpc` (`lower.rs` data-overlaps-`.text` check)
  — separate PR, in progress.
- Any artifact-format change (e.g. the v5 `initial_memory_addr` header field).
- hogd runtime / v4 parse-load (already correct and CI-green).
