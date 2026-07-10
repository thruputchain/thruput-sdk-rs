# Groundhog CLI rv32 Builds + Lowered-Artifact Metrics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route rv32 contract builds through the pinned `groundhog` CLI (which lowers in-process), extend rv32 builds to three integration_test contracts, and add a metric recording the lowered on-chain artifact sizes.

**Architecture:** `scripts/dev.sh`'s `build_rv32()` swaps its raw cargo invocation for `groundhog build --target rv32`; the CLI leaves the unstripped ELF at the existing path (all ELF checks unchanged) and additionally writes `target/groundhog/<snake>.rv32a` + `<snake>.rv32.json`. A new list file `integration_test/rv32-contracts` names the integration_test rv32 set. A new metric script reads the `.rv32.json` sidecars; the existing readelf metric grows to cover the integration_test set. The CLI version is pinned in one sourced shell file; `ci.sh` installs it, local runs only warn.

**Tech Stack:** bash/sh scripts, cargo, groundhog CLI 0.5.14 (groundhog registry), GNU/llvm readelf.

**Spec:** `docs/superpowers/specs/2026-07-09-groundhog-cli-rv32-builds-design.md`

## Global Constraints

- CLI pin: `GROUNDHOG_CLI_VERSION=0.5.14`, defined ONLY in `scripts/groundhog-cli-version.sh`.
- Metric outputs are append-only history: never reorder or rename fields of an existing metric; new metric line format is exactly `<snake> code=<B> initial_memory=<B> memory=<B> artifact=<B>`.
- Metric input sets derive from the source tree (example-contracts `*-rv32` glob + `integration_test/rv32-contracts` list), never from globbing artifact dirs; a missing artifact/sidecar for a current contract must exit 1.
- rv32 ELF checks (`check_rv32_dispatch`, `check_rv32_mem_symbols`, string scan) must keep running on the unstripped ELF at `target/riscv32im-unknown-none-elf/release/<snake>`.
- `groundhog build --target rv32` must be invoked with env `CARGO_PROFILE_RELEASE_STRIP=false` and trailing cargo args `-Zbuild-std=core,alloc` (plus `--locked` unless `CARGO_UNLOCK` is set).
- All work on branch `geoff/groundhog-cli-rv32-builds` (repo hook rejects commits elsewhere).
- Executing this plan requires `groundhog` 0.5.14 on PATH (already installed locally).

---

### Task 1: CLI version pin file + CI install + dev.sh presence check

**Files:**
- Create: `scripts/groundhog-cli-version.sh`
- Modify: `scripts/ci.sh` (currently 3 lines: `#!/bin/bash` / blank / `exec ./scripts/dev.sh ci`)
- Modify: `scripts/dev.sh` (add `require_groundhog_cli()` helper after `fail()`, which ends at line ~743)

**Interfaces:**
- Produces: `scripts/groundhog-cli-version.sh` sets shell var `GROUNDHOG_CLI_VERSION` (no shebang, sourced only). `require_groundhog_cli` (dev.sh function): returns 1 with install hint if `groundhog` absent; warns and returns 0 on version mismatch; silent success on exact match. Tasks 2 and 3 call it.

- [ ] **Step 1: Create the pin file**

Write `scripts/groundhog-cli-version.sh`:

```sh
# Pinned groundhog CLI version.
#
# Single source of truth: sourced by scripts/ci.sh (which installs exactly
# this version before dev.sh ci) and by scripts/dev.sh (which warns when the
# local CLI differs — CI is authoritative for metrics/artifacts). Bump this
# one line when a new groundhog-cli release is published.
GROUNDHOG_CLI_VERSION=0.5.14
```

- [ ] **Step 2: Add the install step to ci.sh**

Replace the full contents of `scripts/ci.sh` with:

```sh
#!/bin/bash

set -e

# Install the pinned groundhog CLI (dev.sh rv32 contract builds run
# `groundhog build --target rv32`, which lowers the ELF in-process).
# The version check makes re-runs with a warm ~/.cargo/bin free; the
# registry index + token are already configured (.cargo/config.toml,
# CARGO_REGISTRIES_GROUNDHOG_TOKEN).
. ./scripts/groundhog-cli-version.sh
if [ "$(groundhog --version 2>/dev/null | awk '{print $2}')" != "$GROUNDHOG_CLI_VERSION" ]; then
    cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
        --registry groundhog --locked
fi

exec ./scripts/dev.sh ci
```

- [ ] **Step 3: Add require_groundhog_cli to dev.sh**

In `scripts/dev.sh`, immediately after the `fail()` function (ends ~line 743, before the `if [[ "${BASH_SOURCE[0]}" == "$0" ]]` block), insert:

```sh
require_groundhog_cli() {
    # rv32 contract builds run through the groundhog CLI. CI installs the
    # exact pin (scripts/ci.sh); locally a mismatch only warns — the pin is
    # authoritative for CI metrics, not a local build requirement.
    . "${SDK_DIR}/scripts/groundhog-cli-version.sh"

    command -v groundhog > /dev/null || {
        fail "groundhog CLI not found; install with: cargo install groundhog-cli --version =${GROUNDHOG_CLI_VERSION} --registry groundhog --locked"
        return 1
    }

    local installed
    installed=$(groundhog --version | awk '{print $2}')
    if [ "$installed" != "$GROUNDHOG_CLI_VERSION" ]; then
        warn "groundhog CLI ${installed} != pinned ${GROUNDHOG_CLI_VERSION} (CI uses the pin; continuing with the local CLI)"
    fi
}
```

- [ ] **Step 4: Verify the helper's three behaviors**

Run from the repo root:

```bash
bash -c 'source scripts/dev.sh; require_groundhog_cli && echo OK-MATCH'
bash -c 'source scripts/dev.sh; groundhog() { echo "groundhog 0.0.1"; }; require_groundhog_cli && echo OK-WARN'
bash -c 'source scripts/dev.sh; PATH=/usr/bin:/bin require_groundhog_cli || echo OK-MISSING'
```

Expected: first prints `OK-MATCH` (no warning, local CLI is 0.5.14); second prints the `>>` mismatch warning then `OK-WARN`; third prints the `!!` install-hint failure then `OK-MISSING`.

Also syntax-check ci.sh without running it: `bash -n scripts/ci.sh` → no output.

- [ ] **Step 5: Commit**

```bash
git add scripts/groundhog-cli-version.sh scripts/ci.sh scripts/dev.sh
git commit -m "ci: pin groundhog CLI version, install in ci.sh, check in dev.sh"
```

---

### Task 2: dev.sh example-contracts rv32 builds go through the CLI

**Files:**
- Modify: `scripts/dev.sh:111-135` (`build_rv32`), `scripts/dev.sh:223-238` (`eg_build_rv32_all`)

**Interfaces:**
- Consumes: `require_groundhog_cli` (Task 1).
- Produces: `build_rv32 <workspace_dir> <package>` now emits, in addition to the unstripped ELF, `target/groundhog/<snake>.rv32a`, `<snake>.rv32.json`, `<snake>.spec.json`, `<snake>.spec.xdr`. Tasks 4–5 rely on those paths.

- [ ] **Step 1: Replace build_rv32's cargo invocation**

Replace the whole `build_rv32()` function (currently lines 111–135) with:

```sh
build_rv32() {
    local workspace_dir
    workspace_dir="$1"; shift
    local package
    package="$1"; shift

    info "BUILD(rv32) $package"

    # --locked matches the cargo() wrapper's default for the CLI's inner
    # cargo invocation; CARGO_UNLOCK opts out, same as everywhere else.
    local locked="--locked"
    if [ -v CARGO_UNLOCK ] && [ -n "$CARGO_UNLOCK" ]; then
        locked=""
    fi

    (
        cd "$workspace_dir" &&
        # The groundhog CLI compiles the ELF (honoring this workspace's
        # .cargo/config.toml rv32 rustflags) and lowers it in-process,
        # writing target/groundhog/<snake>.{rv32a,rv32.json,spec.json,spec.xdr}.
        #
        # build-std passes through as trailing cargo args, exactly like
        # hogd's build_risc.sh: the rv32 rustflags use panic=immediate-abort
        # + target features the prebuilt core lacks. NOT in [unstable]
        # config — that would break host `cargo test` runs from this cwd
        # (build-std requires --target).
        #
        # Build unstripped (env overrides the config profile's
        # strip="symbols"): the symtab does not change .text/.rodata/.data
        # sizes, and it lets the dispatch check use its strong symbol path
        # and check_rv32_mem_symbols assert the compiler_builtins
        # mem-override held. The deployed artifact is the lowered .rv32a,
        # which carries no symtab regardless.
        CARGO_PROFILE_RELEASE_STRIP=false \
        groundhog build --target rv32 -p "$package" -- $locked -Zbuild-std=core,alloc
    )
}
```

- [ ] **Step 2: Gate eg_build_rv32_all on the CLI**

In `eg_build_rv32_all()` add the check as the first line of the function body:

```sh
eg_build_rv32_all() {
    require_groundhog_cli || return 1

    local package elf
    for package in $(ls_rv32_packages "${SDK_DIR}/example-contracts"); do
```

(rest of the function unchanged.)

- [ ] **Step 3: Run the example rv32 build end-to-end**

```bash
rm -f target/groundhog/counter_rv32.rv32a target/groundhog/counter_rv32.rv32.json
./scripts/dev.sh build-examples-rv32
```

Expected: `BUILD(rv32) counter-rv32`, CLI status lines ending `Built .../target/groundhog/counter_rv32.rv32a (<N> bytes)`, then `OK:` lines from the dispatch check, mem-symbol check, and string scan. Exit 0.

- [ ] **Step 4: Verify the lowered artifacts exist**

```bash
ls -l target/groundhog/counter_rv32.rv32a target/groundhog/counter_rv32.rv32.json
grep -E '"(code|artifact|initial_memory|memory)_bytes"' target/groundhog/counter_rv32.rv32.json
```

Expected: both files present; grep shows the four integer fields.

- [ ] **Step 5: Commit**

```bash
git add scripts/dev.sh
git commit -m "dev.sh: build example rv32 contracts via the groundhog CLI"
```

---

### Task 3: integration_test rv32 starter set

**Files:**
- Create: `integration_test/rv32-contracts`
- Modify: `integration_test/Cargo.toml` (members list, lines ~5-35)
- Modify: `integration_test/contracts/rv32_smoke_counter/Cargo.toml` (drop runtime dep)
- Modify: `integration_test/contracts/rv32_smoke_counter/src/main.rs` (drop extern crate)
- Modify: `scripts/dev.sh` — generalize `check_rv32_strings` (lines ~144-170), add `ls_itc_rv32_packages` + `itc_build_rv32_all`, wire into `check_all` (~line 596) and the subcommand dispatch (~line 792)

**Interfaces:**
- Consumes: `build_rv32`, `require_groundhog_cli`, `check_rv32_dispatch`, `check_rv32_mem_symbols` (Tasks 1–2 / existing).
- Produces: `integration_test/rv32-contracts` (one package name per line, `#` comments and blanks ignored) — Tasks 4 and 5 read it. `ls_itc_rv32_packages` (prints package names), `itc_build_rv32_all`, dev.sh subcommand `build-integration-rv32`, skip var `SKIP_RV32_INTEGRATION`. `check_rv32_strings` signature changes to take package names as arguments.

- [ ] **Step 1: Create the list file**

Write `integration_test/rv32-contracts`:

```
# integration_test contracts built as rv32 (via `groundhog build --target
# rv32`, driven by scripts/dev.sh build-integration-rv32). One package name
# per line. Read by dev.sh and by scripts/metrics/sdk_codesizes_rv32*.sh —
# unlike example-contracts, integration_test rv32 contracts are not *-rv32
# wrapper packages (the rv32 bin wrapper is the contract's own src/main.rs),
# so the enabled set is this explicit list.
rv32_smoke_counter
itc_erc20
fibonacci
```

- [ ] **Step 2: Make rv32_smoke_counter a workspace member**

In `integration_test/Cargo.toml`, in the alphabetized `members` list, after `"contracts/return_fault_test",` add:

```toml
    "contracts/rv32_smoke_counter",
```

- [ ] **Step 3: Drop the explicit runtime dependency**

In `integration_test/contracts/rv32_smoke_counter/Cargo.toml`, delete the line:

```toml
groundhog-rv32-runtime = { workspace = true }
```

(The integration_test workspace has no `groundhog-rv32-runtime` entry in `[workspace.dependencies]`, so membership would not even resolve with the dep in place. The runtime — `_start`, panic handler, allocator, linker script — arrives transitively: `sdk` depends on and force-links `groundhog-rv32-runtime` on rv32 targets, the counter-rv32 pattern.)

In `integration_test/contracts/rv32_smoke_counter/src/main.rs`, replace:

```rust
// The runtime crate provides _start, panic handler, allocator, and linker
// script for the RV32 target.
extern crate groundhog_rv32_runtime;

// Link the contract library so its macro-emitted method entry symbols are
// present in `.text.gh_*` for the lowerer to discover.
extern crate rv32_smoke_counter;
```

with:

```rust
// Link the contract library so its macro-emitted method entry symbols are
// linked in for the lowerer to discover. The runtime (_start, panic
// handler, allocator, linker script) arrives transitively via
// rv32_smoke_counter -> sdk, which force-links groundhog-rv32-runtime on
// rv32 targets.
extern crate rv32_smoke_counter;
```

- [ ] **Step 4: Verify host workspace still builds and tests**

```bash
cd integration_test && cargo test --workspace -- --show-output; cd ..
```

Expected: compiles (rv32_smoke_counter's lib now builds for host — no rv32-only dep) and tests pass. If the lib fails host compilation for another reason, STOP and report; do not feature-gate ad hoc.

- [ ] **Step 5: Generalize check_rv32_strings to take package names**

Replace the whole `check_rv32_strings()` function (currently loops example-contracts internally) with:

```sh
check_rv32_strings() {
    # Same invariant check-wasm-size.sh enforces for wasm: production rv32
    # builds (panic=immediate-abort, strip=symbols) must embed no .rs file
    # paths. No allow-list — no rv32 contract builds with `debugging` today;
    # if one ever does, promote this to a script sharing check-wasm-size.sh's
    # allow-list logic rather than forking it here.
    #
    # Takes package names as args; scans exactly those ELFs, not a directory
    # glob: the release dir is shared with root-workspace rv32 builds and
    # can hold stale ELFs from renamed or removed packages.
    info "Checking rv32 binaries for embedded path strings"

    local package elf rs_paths
    for package in "$@"; do
        elf="${RV32_RELEASE}/${package//-/_}"
        [ -f "$elf" ] || {
            fail "missing rv32 ELF $elf (run build-examples-rv32 / build-integration-rv32 first)"
            return 1
        }
        rs_paths=$(strings "$elf" | grep -E '\.rs$' || true)
        if [ -n "$rs_paths" ]; then
            printf '%s\n' "$rs_paths" | sed 's/^/  /'
            fail "$(basename "$elf") contains embedded .rs path strings"
            return 1
        fi
        echo "OK:   $(basename "$elf")"
    done
}
```

Update its one existing caller — the last line of `eg_build_rv32_all` changes from:

```sh
    check_rv32_strings
```

to:

```sh
    check_rv32_strings $(ls_rv32_packages "${SDK_DIR}/example-contracts")
```

- [ ] **Step 6: Add the integration_test rv32 list reader and build loop**

In `scripts/dev.sh`, directly after `eg_build_rv32_all()`, add:

```sh
# integration_test rv32 contracts are NOT *-rv32 wrapper packages (the rv32
# bin wrapper is each contract's own src/main.rs, and nearly every contract
# has one) — the enabled set is the explicit list file, one package per line.
ls_itc_rv32_packages() {
    grep -Ev '^[[:space:]]*(#|$)' "${SDK_DIR}/integration_test/rv32-contracts"
}

itc_build_rv32_all() {
    require_groundhog_cli || return 1

    local package elf
    for package in $(ls_itc_rv32_packages); do
        build_rv32 "${SDK_DIR}/integration_test" "$package" || {
            fail "Failed building rv32 contract $package"
            return 1
        }
        elf="${RV32_RELEASE}/${package//-/_}"
        check_rv32_dispatch "$elf" || return 1
        check_rv32_mem_symbols "$elf" || return 1
    done
    check_rv32_strings $(ls_itc_rv32_packages)
}
```

- [ ] **Step 7: Wire into check_all and the subcommand dispatch**

In `check_all()`, the INTEGRATION block becomes:

```sh
    { skip INTEGRATION || {
        itc_build_all &&
        { skip RV32_INTEGRATION || itc_build_rv32_all; } &&
        itc_test_all &&
        itc_bottle_test_all
    }; } &&
```

In the bottom `case` dispatch, after the `build-itc)` entry add:

```sh
        build-integration-rv32)
            itc_build_rv32_all
            ;;
```

- [ ] **Step 8: Run the integration_test rv32 build end-to-end**

```bash
./scripts/dev.sh build-integration-rv32
```

Expected: `BUILD(rv32)` for `rv32_smoke_counter`, `itc_erc20`, `fibonacci`; per-ELF `OK:` lines from dispatch, mem-symbol, and string checks; exit 0. Then:

```bash
ls -l target/groundhog/rv32_smoke_counter.rv32a target/groundhog/itc_erc20.rv32a target/groundhog/fibonacci.rv32a
```

Expected: all three artifacts (plus `.rv32.json` sidecars) present.

- [ ] **Step 9: Commit**

```bash
git add integration_test/rv32-contracts integration_test/Cargo.toml \
    integration_test/Cargo.lock \
    integration_test/contracts/rv32_smoke_counter/Cargo.toml \
    integration_test/contracts/rv32_smoke_counter/src/main.rs scripts/dev.sh
git commit -m "integration_test: rv32 builds via groundhog CLI for a starter contract set"
```

---

### Task 4: New metric sdk_codesizes_rv32_lowered

**Files:**
- Create: `scripts/metrics/sdk_codesizes_rv32_lowered.sh` (mode 755, like siblings)
- Modify: `scripts/metrics.sh` (add one `produce_metric` call after the `sdk_codesizes_rv32` one)

**Interfaces:**
- Consumes: `target/groundhog/<snake>.rv32.json` sidecars (Task 2/3), `integration_test/rv32-contracts` (Task 3).
- Produces: metric `sdk_codesizes_rv32_lowered`, one line per contract: `<snake> code=<B> initial_memory=<B> memory=<B> artifact=<B>`.

- [ ] **Step 1: Write the metric script**

Write `scripts/metrics/sdk_codesizes_rv32_lowered.sh`:

```sh
#!/bin/bash
set -euo pipefail

# Per-contract lowered rv32 artifact metrics: the true on-chain numbers,
# read from the `groundhog build --target rv32` metadata sidecars
# (<name>.rv32.json in target/groundhog/). sdk_codesizes_rv32 keeps the
# per-section detail of the unstripped ELF; this metric records what the
# lowerer actually emits.
#
# Output format (one line per contract, append-only metric history —
# do not reorder or rename fields):
#   <name> code=<B> initial_memory=<B> memory=<B> artifact=<B>
#
#   code           = lowered code image bytes         (sidecar code_bytes)
#   initial_memory = stored initial data image bytes  (initial_memory_bytes)
#   memory         = total VM memory reservation      (memory_bytes)
#   artifact       = full .rv32a artifact size        (artifact_bytes)

if [ $# -lt 1 ] || [ $# -gt 3 ]; then
  echo "Usage: $0 <groundhog artifact dir> [example contracts dir] [itc rv32 list]" >&2
  exit 1
fi

ARTIFACT_DIR=$1
# The metric input set is derived from the source tree (example-contracts
# *-rv32 wrapper packages + the integration_test rv32-contracts list), NOT
# from globbing the artifact dir: target/groundhog/ is shared across both
# contract workspaces and can hold stale artifacts from renamed or removed
# contracts — none of those may emit a row into the append-only history,
# and a missing sidecar for a current contract must fail loudly.
CONTRACTS_DIR=${2:-example-contracts}
ITC_RV32_LIST=${3:-integration_test/rv32-contracts}

json_int() {
    # Integer field $2 from sidecar $1. The sidecar is generated with
    # sorted keys and 2-space pretty-printing by the pinned groundhog CLI,
    # so line-based extraction is stable; fail loudly rather than emit a
    # missing/empty value into append-only history.
    local val
    val=$(sed -n "s/^ *\"$2\": \([0-9][0-9]*\),*$/\1/p" "$1")
    [ -n "$val" ] || {
        echo "field $2 missing from $1" >&2
        exit 1
    }
    echo "$val"
}

emit_row() {
    local name=$1
    local sidecar="$ARTIFACT_DIR/$name.rv32.json"
    if [ ! -f "$sidecar" ]; then
        echo "Expected lowered rv32 sidecar missing: $sidecar (build with scripts/dev.sh build-examples-rv32 / build-integration-rv32)" >&2
        exit 1
    fi
    echo "$name" \
        "code=$(json_int "$sidecar" code_bytes)" \
        "initial_memory=$(json_int "$sidecar" initial_memory_bytes)" \
        "memory=$(json_int "$sidecar" memory_bytes)" \
        "artifact=$(json_int "$sidecar" artifact_bytes)"
}

names=""
for pkg_dir in "$CONTRACTS_DIR"/*-rv32; do
    [ -d "$pkg_dir" ] && [ -f "$pkg_dir/Cargo.toml" ] || continue
    name=$(basename "$pkg_dir")
    names="$names ${name//-/_}"
done
if [ -f "$ITC_RV32_LIST" ]; then
    for name in $(grep -Ev '^[[:space:]]*(#|$)' "$ITC_RV32_LIST"); do
        names="$names ${name//-/_}"
    done
fi

if [ -z "$names" ]; then
    echo "No rv32 contracts found ($CONTRACTS_DIR, $ITC_RV32_LIST)" >&2
    exit 1
fi

# target/groundhog/ is one flat namespace shared by both workspaces: a
# duplicate snake name would silently attribute one contract's numbers to
# the other, so fail loudly instead.
seen=" "
for name in $names; do
    case "$seen" in
        *" $name "*)
            echo "duplicate rv32 contract name: $name" >&2
            exit 1
            ;;
    esac
    seen="$seen$name "
    emit_row "$name"
done
```

Then: `chmod 755 scripts/metrics/sdk_codesizes_rv32_lowered.sh`

- [ ] **Step 2: Run it against the built artifacts (happy path)**

```bash
./scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog
```

Expected output — exactly four lines (order: example glob first, then list order), each like:

```
counter_rv32 code=5396 initial_memory=24720 memory=65536 artifact=30248
rv32_smoke_counter code=... initial_memory=... memory=... artifact=...
itc_erc20 code=... initial_memory=... memory=... artifact=...
fibonacci code=... initial_memory=... memory=... artifact=...
```

(byte values will differ; format must match exactly.)

- [ ] **Step 3: Verify the loud-failure path**

```bash
mv target/groundhog/fibonacci.rv32.json /tmp/fib.json.bak
./scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog; echo "exit=$?"
mv /tmp/fib.json.bak target/groundhog/fibonacci.rv32.json
```

Expected: `Expected lowered rv32 sidecar missing: ...fibonacci.rv32.json ...` on stderr and `exit=1`.

- [ ] **Step 4: Wire into metrics.sh**

In `scripts/metrics.sh`, after:

```sh
# METRIC: sdk_codesizes_rv32
produce_metric sdk_codesizes_rv32 "target/riscv32im-unknown-none-elf/release"
```

add:

```sh
# METRIC: sdk_codesizes_rv32_lowered
produce_metric sdk_codesizes_rv32_lowered "target/groundhog"
```

Syntax-check (metrics.sh runs under dash on CI): `sh -n scripts/metrics.sh` → no output.

- [ ] **Step 5: Commit**

```bash
git add scripts/metrics/sdk_codesizes_rv32_lowered.sh scripts/metrics.sh
git commit -m "metrics: add sdk_codesizes_rv32_lowered from CLI lowering sidecars"
```

---

### Task 5: Extend sdk_codesizes_rv32 (ELF detail) to the integration_test set

**Files:**
- Modify: `scripts/metrics/sdk_codesizes_rv32.sh`

**Interfaces:**
- Consumes: `integration_test/rv32-contracts` (Task 3), ELFs from Tasks 2–3.
- Produces: same line format as today (`<name> text=<B> rodata=<B> data=<B> bss=<B> data_image=<B> file=<B>`), now with rows for the listed integration_test contracts. Output format unchanged — only the input set grows.

- [ ] **Step 1: Factor the row emission and add the list-driven loop**

In `scripts/metrics/sdk_codesizes_rv32.sh`:

(a) Change the usage/arg block from `[ $# -lt 1 ] || [ $# -gt 2 ]` to accept an optional third arg, and add the list default after `CONTRACTS_DIR`:

```sh
if [ $# -lt 1 ] || [ $# -gt 3 ]; then
  echo "Usage: $0 <elf directory> [contracts dir] [itc rv32 list]"
  exit 1
fi
```

```sh
CONTRACTS_DIR=${2:-example-contracts}
# integration_test rv32 contracts are not *-rv32 wrapper packages; the
# enabled set is the explicit list file (one package name per line, '#'
# comments allowed). Same source-tree-derived input rationale as above.
ITC_RV32_LIST=${3:-integration_test/rv32-contracts}
```

(b) After the `section_size()` function, add a row helper capturing the current loop body:

```sh
emit_row() {
    # $1 = snake-case contract/bin name; ELF must exist in $TARGET_DIR.
    local name elf text rodata data bss file
    name=$1
    elf="$TARGET_DIR/$name"
    if [ ! -f "$elf" ] || [ ! -x "$elf" ]; then
        echo "Expected rv32 contract ELF missing: $elf (build with scripts/dev.sh build-examples-rv32 / build-integration-rv32)" >&2
        exit 1
    fi
    text=$(section_size "$elf" .text)
    rodata=$(section_size "$elf" .rodata)
    data=$(section_size "$elf" .data)
    bss=$(section_size "$elf" .bss)
    file=$(wc -c < "$elf" | tr -d ' ')
    echo "$name text=$text rodata=$rodata data=$data bss=$bss data_image=$((rodata + data)) file=$file"
}
```

(c) Replace the existing `for pkg_dir ...` loop body's ELF/readelf logic with calls to the helper, and append the list loop:

```sh
found=0
for pkg_dir in "$CONTRACTS_DIR"/*-rv32; do
    [ -d "$pkg_dir" ] && [ -f "$pkg_dir/Cargo.toml" ] || continue
    found=1
    # Bin name = underscored package name by convention (the wrapper's
    # [[bin]] declares it; see dev.sh eg_build_rv32_all).
    name=$(basename "$pkg_dir")
    emit_row "${name//-/_}"
done

if [ -f "$ITC_RV32_LIST" ]; then
    for name in $(grep -Ev '^[[:space:]]*(#|$)' "$ITC_RV32_LIST"); do
        found=1
        emit_row "${name//-/_}"
    done
fi

if [ "$found" -eq 0 ]; then
    echo "No *-rv32 wrapper packages found under $CONTRACTS_DIR" >&2
    exit 1
fi
```

(d) Update the header comment's format note to mention that input rows now come from both the example-contracts `*-rv32` glob and the integration_test list file (format unchanged).

- [ ] **Step 2: Run it and compare against the pre-change output shape**

```bash
./scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release
```

Expected: four rows (`counter_rv32`, `rv32_smoke_counter`, `itc_erc20`, `fibonacci`), each with the exact existing field set `text= rodata= data= bss= data_image= file=`. Confirm the `counter_rv32` row's values are identical to running the pre-change script (`git stash; ./scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release; git stash pop` — or just eyeball against the values from Task 2's build).

- [ ] **Step 3: Commit**

```bash
git add scripts/metrics/sdk_codesizes_rv32.sh
git commit -m "metrics: include integration_test rv32 contracts in sdk_codesizes_rv32"
```

---

### Task 6: Full verification sweep

**Files:** none (verification only)

- [ ] **Step 1: Clean rebuild of both rv32 sets**

```bash
rm -rf target/groundhog
./scripts/dev.sh build-examples-rv32 && ./scripts/dev.sh build-integration-rv32
```

Expected: both succeed; all dispatch/mem/strings `OK:` lines; `target/groundhog/` repopulated with 4 `.rv32a` + 4 `.rv32.json` (+ spec files).

- [ ] **Step 2: Both metric scripts green**

```bash
./scripts/metrics/sdk_codesizes_rv32.sh target/riscv32im-unknown-none-elf/release
./scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog
```

Expected: 4 well-formed rows each, no stderr.

- [ ] **Step 3: Host tests and wasm path unaffected**

```bash
(cd integration_test && cargo test --workspace -- --show-output)
./scripts/dev.sh build-itc
```

Expected: tests pass; wasm builds (fibonacci + contracts dir) succeed exactly as before — rv32 changes must not touch the wasm loop.

- [ ] **Step 4: Shell syntax checks**

```bash
bash -n scripts/dev.sh scripts/ci.sh scripts/metrics/sdk_codesizes_rv32.sh scripts/metrics/sdk_codesizes_rv32_lowered.sh
sh -n scripts/metrics.sh
```

Expected: no output.

- [ ] **Step 5: Commit any stragglers (e.g. Cargo.lock) and report**

```bash
git status --short
```

Expected: clean (or only intentional files). Report results; branch ready for `superpowers:finishing-a-development-branch`.
