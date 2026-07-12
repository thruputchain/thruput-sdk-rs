# RV32 gp Relaxation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable linker gp (global-pointer) relaxation for rv32 contracts so static accesses within ±2 KiB of an anchor compile to a single `lw/sw off(gp)` instead of a `lui+addi` pair plus access — smaller code and one less executed instruction (= 1 gas) per relaxed access.

**Architecture:** Three cooperating pieces, useless in isolation: (1) the linker script defines the anchor symbol `__global_pointer$` at `__stack_top + 0x800` (statics start exactly at `__stack_top` in our stack-first layout, so a ±2 KiB window centered 0x800 past it covers the first 4 KiB of statics); (2) `_start` initializes the `gp` register from that symbol before any static access, wrapped in `.option norelax` so the init itself isn't circularly relaxed; (3) the contract rustflags pass `--relax-gp`, lld's explicit opt-in (unlike GNU ld, lld does no gp relaxation by default). `-C target-feature=+relax` (which emits the `R_RISCV_RELAX` relocations lld needs) is **already present** — do not re-add it. Relaxation is purely opportunistic per access: out-of-window symbols keep today's code exactly; correctness is never affected. The VM never touches gp — it's just a guest register.

**Tech Stack:** rust nightly-style flags via `.cargo/config.toml`, lld (`rust-lld`), `groundhog build --target rv32` (installed groundhog CLI), hogd gtest smoke for execution verification.

## Global Constraints

- **Never commit on main.** The repo hook blocks it. Create the branch with `git checkout -b geoff/rv32-gp-relaxation` as its own standalone command (not chained with `&&`) before any commit.
- **The rv32 rustflags stanza exists in TWO synced copies**: `example-contracts/.cargo/config.toml` (canonical) and `integration_test/.cargo/config.toml` (synced copy). Every rustflags edit goes in BOTH, keeping the lists byte-identical.
- **NEVER add rv32 rustflags to the workspace-root `.cargo/config.toml`** — cargo joins target-rustflags along the config-discovery chain and a duplicated `-Tgroundhog-rv32.ld` fails the link ("region 'CODE' already defined").
- rv32 contract builds require the groundhog CLI on PATH (`groundhog --version` to check; `./scripts/groundhog-cli-version.sh` names the pinned version) and are run via `./scripts/dev.sh build-examples-rv32` / `build-integration-rv32` — never bare `cargo build` (build-std flags come from dev.sh).
- **Measure-first gate:** the change ships only if the after-measurement shows `.text` shrinking (or at worst flat) with the smoke test passing. If `.text` grows, stop and report — do not commit.
- All paths below are relative to the SDK repo root `/Users/geofframseyer/groundhog/agent1/sdk_rs` unless absolute.
- Scratch files go in the session scratchpad, referenced below as `$SCRATCH` — set `SCRATCH=/private/tmp/claude-501/-Users-geofframseyer-groundhog-agent1-sdk-rs/c6557659-8023-459c-ae00-3530e1ef5c84/scratchpad` in each shell that uses it.

---

### Task 1: Branch and baseline measurement

**Files:**
- Create: `$SCRATCH/baseline_rv32_sections.txt`, `$SCRATCH/baseline_rv32_lowered.txt` (scratch only, never committed)

**Interfaces:**
- Produces: the two baseline metric files Task 3 diffs against; the branch `geoff/rv32-gp-relaxation` all commits land on.

- [ ] **Step 1: Create the working branch (standalone command)**

```bash
git checkout -b geoff/rv32-gp-relaxation
```

Expected: `Switched to a new branch 'geoff/rv32-gp-relaxation'`

- [ ] **Step 2: Build all rv32 contracts at the current baseline**

```bash
./scripts/dev.sh build-examples-rv32
./scripts/dev.sh build-integration-rv32
```

Expected: each contract prints `BUILD(rv32) <pkg>` and finishes; the example loop also runs the dispatch/mem-symbol/string checks (`OK: ... uses guest-side dispatch`). Requires the pinned groundhog CLI; if `groundhog` is missing, stop and report rather than improvising an install.

- [ ] **Step 3: Record baseline metrics**

```bash
SCRATCH=/private/tmp/claude-501/-Users-geofframseyer-groundhog-agent1-sdk-rs/c6557659-8023-459c-ae00-3530e1ef5c84/scratchpad
./scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release > "$SCRATCH/baseline_rv32_sections.txt"
./scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog > "$SCRATCH/baseline_rv32_lowered.txt"
cat "$SCRATCH/baseline_rv32_sections.txt"
```

Expected: one `<name> text=... rodata=... data=... bss=... data_image=... file=...` row per contract (counter_rv32, the integration contracts, etc.), non-zero text sizes.

- [ ] **Step 4: Confirm the baseline has zero gp-relative accesses**

```bash
/opt/homebrew/opt/llvm/bin/llvm-objdump -d target/riscv32im-unknown-none-elf/release/counter_rv32 | grep -c '(gp)'
```

Expected: `0` (grep exits 1 on zero matches — that exit code is the expected outcome, not an error). This is the audit's "silent no-op" tripwire in reverse: after the change this count must be > 0.

---

### Task 2: The three-piece change (anchor, gp init, link flag)

**Files:**
- Modify: `crates/rv32-runtime/link/groundhog-rv32.ld` (after the `__stack_top` definition, ~line 99)
- Modify: `crates/rv32-runtime/src/start.rs:84-88` (the `naked_asm!` block)
- Modify: `example-contracts/.cargo/config.toml` (canonical rv32 stanza, ~line 56 after `--icf=all`, plus the header comment ~line 26)
- Modify: `integration_test/.cargo/config.toml` (synced rv32 stanza, ~line 31 after `--icf=all`)

**Interfaces:**
- Consumes: `__stack_top` (already defined in the linker script), `-C target-feature=+relax` (already in both rustflags stanzas — leave untouched).
- Produces: link-time symbol `__global_pointer$ = __stack_top + 0x800` (PROVIDE, so a contract `--defsym` can override); `gp` live from VM entry onward. Task 3 verifies both.

- [ ] **Step 1: Add the anchor to the linker script**

In `crates/rv32-runtime/link/groundhog-rv32.ld`, directly after the line `__stack_top = __stack_bottom + __stack_size;` (keeping the blank line before `SECTIONS`), insert:

```ld
/* gp-relaxation anchor. With --relax-gp (set in the contract rustflags),
 * lld rewrites the lui+addi address materialization of any symbol whose
 * final address lands in [gp - 0x800, gp + 0x7FF] into a single
 * gp-relative instruction. Statics start exactly at __stack_top
 * (stack-first layout), so anchoring 0x800 past it covers the first
 * 4 KiB of .rodata/.data/.bss — for small contracts, the whole statics
 * band. Purely opportunistic: out-of-window accesses keep the
 * two-instruction form; correctness is never involved. _start loads gp
 * from this symbol (rv32-runtime/src/start.rs). PROVIDE yields to a
 * per-contract --defsym override, and the expression tracks any
 * __stack_size override automatically. */
PROVIDE(__global_pointer$ = __stack_top + 0x800);
```

- [ ] **Step 2: Initialize gp in `_start`**

In `crates/rv32-runtime/src/start.rs`, replace the `naked_asm!` call:

```rust
    naked_asm!(
        // Load gp with the linker-script relaxation anchor BEFORE anything
        // can touch a static. The norelax bracket keeps this la itself from
        // being relaxed into a gp-relative move — gp isn't set yet.
        ".option push",
        ".option norelax",
        "la gp, __global_pointer$",
        ".option pop",
        "la sp, __stack_top",
        "tail {dispatch}",
        dispatch = sym rv32_dispatch,
    );
```

(The existing comment block above the `naked_asm!` about sp stays as is; only the asm changes.)

Also extend the doc comment at the top of the function body's existing comment — after the sentence ending "so no rounding here.", add one line:

```rust
    // gp is loaded first so every subsequent static access is eligible for
    // the linker's gp relaxation (see link/groundhog-rv32.ld).
```

- [ ] **Step 3: Add `--relax-gp` to the canonical rustflags stanza**

In `example-contracts/.cargo/config.toml`, after the `"-C", "link-arg=--icf=all",` entry, insert:

```toml
    # gp relaxation: with the __global_pointer$ anchor in groundhog-rv32.ld
    # and the gp init in rv32-runtime _start, lld folds static-address
    # lui+addi pairs into single gp-relative accesses for statics within
    # +/-2 KiB of the anchor. lld needs this explicit opt-in (GNU ld
    # relaxes gp by default; lld deliberately doesn't).
    "-C", "link-arg=--relax-gp",
```

And in the stanza's header comment, update the sentence

`# rv32 build) plus `+relax` and `--icf=all` (measured wins; see`

to

`# rv32 build) plus `+relax`, `--icf=all`, and `--relax-gp` (measured wins;`
`# see docs/superpowers/specs/2026-07-09-rv32-build-config-ci-design.md).`

(keeping the doc reference intact on the following line — adjust line wrapping as needed).

- [ ] **Step 4: Mirror the flag into the synced copy**

In `integration_test/.cargo/config.toml`, after `"-C", "link-arg=--icf=all",` insert exactly:

```toml
    "-C", "link-arg=--relax-gp",
```

(This file deliberately carries no per-flag comments — its header defers to the canonical file. The two rustflags ARRAYS must end up element-for-element identical; run `diff <(grep '"-' example-contracts/.cargo/config.toml) <(grep '"-' integration_test/.cargo/config.toml)` and expect only the wasm-stanza context to differ — the rv32 flag lists must match.)

- [ ] **Step 5: Rebuild everything**

```bash
./scripts/dev.sh build-examples-rv32
./scripts/dev.sh build-integration-rv32
```

Expected: all contracts build AND lower cleanly (the CLI lowers in-process, so a layout violation would fail right here), and the example loop's dispatch/mem/strings checks still print `OK`.

---

### Task 3: Verify, measure, smoke, commit

**Files:**
- Create: `$SCRATCH/after_rv32_sections.txt`, `$SCRATCH/after_rv32_lowered.txt` (scratch only)

**Interfaces:**
- Consumes: baseline files from Task 1; built ELFs in `target/riscv32im-unknown-none-elf/release/` and lowered artifacts in `target/groundhog/` from Task 2 Step 5.

- [ ] **Step 1: Verify the anchor resolved where expected**

```bash
/opt/homebrew/opt/llvm/bin/llvm-nm target/riscv32im-unknown-none-elf/release/counter_rv32 | grep global_pointer
```

Expected: `00006800 A __global_pointer$` — default `__stack_size` is 24K (0x6000), so `__stack_top + 0x800 = 0x6800`. Any other value means a stack-size override or a broken expression; investigate before proceeding.

- [ ] **Step 2: Verify relaxation actually fired (the audit's no-op tripwire)**

```bash
/opt/homebrew/opt/llvm/bin/llvm-objdump -d target/riscv32im-unknown-none-elf/release/counter_rv32 | grep -c '(gp)'
```

Expected: a number **greater than 0**. If it's 0, gp relaxation silently did nothing — check that all three pieces landed (`--relax-gp` in the flags actually used, anchor defined, `+relax` still present) and stop rather than commit a no-op.

- [ ] **Step 3: After-measurement and comparison**

```bash
SCRATCH=/private/tmp/claude-501/-Users-geofframseyer-groundhog-agent1-sdk-rs/c6557659-8023-459c-ae00-3530e1ef5c84/scratchpad
./scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release > "$SCRATCH/after_rv32_sections.txt"
./scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog > "$SCRATCH/after_rv32_lowered.txt"
diff "$SCRATCH/baseline_rv32_sections.txt" "$SCRATCH/after_rv32_sections.txt"
diff "$SCRATCH/baseline_rv32_lowered.txt" "$SCRATCH/after_rv32_lowered.txt"
```

Expected: `text=` (and lowered `code=`) shrinks for most contracts. Note the gp init costs 2 instructions (8 B) of `.text` per contract, so a contract with almost no in-window static accesses may be ~flat. **Gate:** if any contract's text GROWS by more than the 8 B init cost, or the aggregate is a regression, stop and report the numbers instead of committing. Record the per-contract before/after deltas — they go in the commit message.

- [ ] **Step 4: Execution smoke on the hogd VM**

The hogd repo has an end-to-end smoke (`LoweredSdkArtifactSmoke` in `rv32_runtime_context_test`) that loads any lowered `.rv32a` via env override, invokes `get`, and asserts a clean Return:

```bash
RV32_SMOKE_ARTIFACT=/Users/geofframseyer/groundhog/agent1/sdk_rs/target/groundhog/rv32_smoke_counter.rv32a \
  /Users/geofframseyer/groundhog/agent1/hogd/build_release/hogd/hogexec/runtime/rv32_runtime_context_test \
  --gtest_filter='*LoweredSdkArtifactSmoke*'
```

Expected: `[smoke] frame status_code=... (Return)` and `[  PASSED  ] 1 test.` If the prebuilt test binary is stale/won't run, rebuild it first: `cmake --build /Users/geofframseyer/groundhog/agent1/hogd/build_release -j16 --target rv32_runtime_context_test`. If the smoke FAILS with the new artifact but the build checks passed, that's a real VM/SDK gap — stop, capture the trace output, and report; do not commit.

- [ ] **Step 5: Commit**

```bash
git add crates/rv32-runtime/link/groundhog-rv32.ld crates/rv32-runtime/src/start.rs example-contracts/.cargo/config.toml integration_test/.cargo/config.toml
git commit -m "rv32: enable linker gp relaxation

Anchor __global_pointer\$ at __stack_top + 0x800 (PROVIDE, so per-contract
overrides and __stack_size changes track automatically), initialize gp in
_start under .option norelax, and pass --relax-gp (lld's explicit opt-in).
Static accesses within +/-2 KiB of the anchor drop their lui+addi
materialization pair: 1 instruction of codesize and 1 gas per access.
Costs a fixed 2-instruction gp init per VM entry.

<per-contract text= before/after deltas from Task 3 Step 3 here>"
```

Expected: clean commit on `geoff/rv32-gp-relaxation`. (The `\$` escape matters inside the double-quoted message.)

- [ ] **Step 6: Report**

Summarize: per-contract text/code deltas, the `(gp)` access count for counter, smoke result. Explicitly note what was NOT measured: per-call executed-instruction (gas) delta — no rig for that yet; the smoke only proves correctness.

---

## Out of scope (deliberate)

- `hogd/built_contracts/.cargo/config.toml` (production build in the hogd repo) — already diverges by `+relax`/`--icf=all`; syncing it is a hogd-repo change with its own measurement.
- `.github/workflows/publish_standard_interfaces.yml`'s minimal link-flag env — that build only needs to LINK the spec-carrier bin; without `--relax-gp` it just skips the optimization, and the PROVIDE'd anchor keeps `_start`'s `la gp` resolvable there. No change needed.
- Per-call gas measurement — no instruction-count rig exists yet; codesize is the measured gate.
