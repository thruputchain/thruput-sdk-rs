# RV32 `.text` shadow elimination (SDK side) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop shipping the code-sized zero "shadow" in the RV32 data image by giving guest data its own low-VMA address space in the SDK linker script.

**Architecture:** The VM is modified-Harvard — CODE (PC) and DATA (load/store) are independent address spaces. Split the linker's single `RAM` region into two MEMORY regions whose VMAs intentionally overlap: `.text` stays in `CODE` at VMA 0; data sections move into a `DATA` region starting at a minimal `0x8` null guard. LLD's overlap check is disabled (`--no-check-sections`) because the overlap is nominal. The data image then contains only real data (~a few hundred bytes) instead of a code-sized leading-zero hole.

**Tech Stack:** GNU-flavor `rust-lld` linker script, RISC-V `riscv32im-unknown-none-elf` target, Cargo `.cargo/config.toml` rustflags.

**Cross-repo dependency:** The matching lowerer relaxation (`groundhog-rpc` `crates/rv32/lower/src/lower.rs:140`, which rejects data sections with `addr < text_end`) is a SEPARATE PR, in progress by the repo owner. Until it lands, ELFs from the new script will not lower, so the end-to-end `.rv32a` shrink and the hogd smoke tests are verified on that PR, not here. **Stop and hand off at Task 3.**

---

### Task 1: Add `--no-check-sections` to the RV32 link flags

This must land before the linker-script change, or the overlapping-VMA link fails. On its own (old single-region script) the flag is a harmless no-op.

**Files:**
- Modify: `example-contracts/.cargo/config.toml` (the `[target.riscv32im-unknown-none-elf]` rustflags array)

- [ ] **Step 1: Add the flag with an explanatory comment**

Insert immediately after the `-Tgroundhog-rv32.ld` link-arg:

```toml
    "-C", "link-arg=-Tgroundhog-rv32.ld",
    # CODE and DATA live in two MEMORY regions whose VMAs intentionally overlap
    # (the VM's Harvard split keeps them in separate address spaces). LLD's
    # default overlap check would abort the link, so disable it.
    "-C", "link-arg=--no-check-sections",
    "-C", "link-arg=-static",
```

- [ ] **Step 2: Commit**

```bash
git add example-contracts/.cargo/config.toml
git commit -m "rv32: allow intentional CODE/DATA VMA overlap in contract links"
```

---

### Task 2: Rewrite the linker script with split CODE/DATA regions

**Files:**
- Modify: `crates/rv32-runtime/link/groundhog-rv32.ld` (full rewrite of header comment, `MEMORY`, and section `> RAM` → `> CODE`/`> DATA` assignments)

- [ ] **Step 1: Replace the `MEMORY` block**

```
MEMORY
{
  CODE (rx) : ORIGIN = 0,   LENGTH = 64K
  DATA (rw) : ORIGIN = 0x8, LENGTH = 64K - 0x8
}
```

- [ ] **Step 2: Point `.text` at CODE, all data sections at DATA**

- `.text 0 : ALIGN(4) { ... } > CODE` (KEEP of `.text._start` unchanged)
- `.rodata`, `.data`, `.bss` blocks: change `> RAM` to `> DATA`. Keep the linkme
  `GROUNDHOG_RV32_ENTRYPOINTS` encapsulation inside `.rodata` exactly as-is.
- Drop the leading `. = ALIGN(8);` that preceded `.rodata` (it was sequencing
  within the single region; `.rodata` now starts the DATA region at its origin).

- [ ] **Step 3: Recompute heap/stack symbols off DATA**

```
  . = ALIGN(8);
  __heap_base = .;
  __stack_top = ORIGIN(DATA) + LENGTH(DATA);
  __stack_bottom = __stack_top - __stack_size;
  __heap_end = __stack_bottom;
```

Keep `ASSERT(__heap_base <= __heap_end, "rv32 image leaves no room for heap/stack");`
and the `/DISCARD/` block unchanged.

- [ ] **Step 4: Rewrite the header comment block**

Document: the Harvard split and intentional CODE/DATA VMA overlap; that
`--no-check-sections` is therefore required; the `0x8` null guard rationale
(no `static` at address 0, so no null `&'static`; 0x8 is one `.rodata` alignment
unit, ~8 leading-zero bytes vs. the old code-sized shadow). Keep the existing
list of lowerer-enforced invariants, updated so data lives "in DATA at low
addresses" rather than "above .text".

(The complete file content is in the working tree from the planning de-risk pass —
see `git diff crates/rv32-runtime/link/groundhog-rv32.ld`.)

- [ ] **Step 5: Build counter-rv32 and verify it links**

```bash
cd example-contracts
cargo build --release -p counter-rv32 --features entrypoints \
  --target riscv32im-unknown-none-elf
```
Expected: `Finished` with no `rust-lld: error: ... overlaps` line.

- [ ] **Step 6: Verify the section layout shrank**

```bash
RE=/opt/homebrew/opt/llvm/bin/llvm-readelf
ELF=../target/riscv32im-unknown-none-elf/release/counter_rv32
$RE -S "$ELF" | grep -E '\.text |\.rodata|\.data |\.bss'
$RE -h "$ELF" | grep Entry
```
Expected: `.text` at `0x0`; `.rodata` at `0x8` (not above the code); `.data`/`.bss`
just after it (data ends in the low hundreds of bytes); Entry point `0x0`.

- [ ] **Step 7: Commit**

```bash
git add crates/rv32-runtime/link/groundhog-rv32.ld
git commit -m "rv32: split CODE/DATA address spaces to drop the .text data shadow"
```

---

### Task 3: Verification sweep + lowerer handoff

**Files:** none (verification only)

- [ ] **Step 1: Confirm guest-side dispatch invariants still hold**

```bash
OBJDUMP=/opt/homebrew/opt/llvm/bin/llvm-objdump \
READOBJ=/opt/homebrew/opt/llvm/bin/llvm-readobj \
NM=/opt/homebrew/opt/llvm/bin/llvm-nm \
scripts/check-rv32-dispatch.sh \
  target/riscv32im-unknown-none-elf/release/counter_rv32
```
Expected: no `FAIL` (a `WARN` about the stripped-ELF path is normal for release).

- [ ] **Step 2: Confirm the runtime crate still builds for the target**

(counter-rv32 transitively builds `groundhog-rv32-runtime`; the successful link in
Task 2 already covers `__stack_top`/`__heap_base`/`__heap_end` resolution.)

- [ ] **Step 3: Check the integration_test/contracts submodule**

Confirm it does not link RV32 contracts with its own cargo config (its
`integration_test/.cargo/config.toml` is wasm-only). If it does build RV32, the
same `--no-check-sections` flag is needed there — a submodule coordination change
outside this PR's direct edits; flag it to the owner.

- [ ] **Step 4: STOP — hand off to the lowerer PR**

The remaining verification needs the updated `groundhog-rpc` lowerer:
- the actual `.rv32a` data-image shrink from ~12.7 KB to a few hundred bytes;
- the hogd runtime-context smoke tests (`increment`/`get`, `LoweredSdkArtifactSmoke`)
  passing unchanged (proves data pointers still resolve under the new layout).

Notify the user that the SDK side is complete and verified to the extent possible
without the lowerer.

---

## Self-Review

- **Spec coverage:** Change 1 (linker script) → Task 2; Change 2 (`--no-check-sections`) → Task 1; SDK-side verification → Tasks 2–3; deferred lowerer verification → Task 3 Step 4; submodule open item → Task 3 Step 3. All spec sections covered.
- **Placeholder scan:** No TBD/TODO; every code/command step has concrete content and expected output.
- **Consistency:** Symbol names (`__stack_top`, `__heap_base`, `__heap_end`, `__bss_start/_end`), region names (`CODE`/`DATA`), and the `0x8` guard are consistent across tasks and match the runtime's `extern` bindings in `src/allocator.rs` and `src/start.rs`.
