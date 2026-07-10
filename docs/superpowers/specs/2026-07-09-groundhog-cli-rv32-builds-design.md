# Groundhog CLI rv32 builds + lowered-artifact metrics

Date: 2026-07-09
Status: approved

## Goal

Build the repo's rv32 contracts through the released `groundhog` CLI (which
runs the RV32 lowerer in-process), extend rv32 builds to a starter set of
integration_test contracts, and record the lowered on-chain artifact sizes as
a new CI metric — alongside, not replacing, the existing per-section ELF
detail metric. The CLI version is pinned in the CI shell scripts.

## Background

- Today `scripts/dev.sh` builds example-contracts `*-rv32` wrappers with raw
  `cargo build -Zbuild-std=core,alloc` and never lowers them; the comment in
  `build_rv32()` notes the on-chain artifact is produced by hogd's pipeline.
  integration_test has no rv32 builds at all.
- `groundhog build --target rv32 -p <pkg>` (CLI ≥ 0.5.x, from groundhog-rpc)
  compiles the ELF honoring the workspace's `.cargo/config.toml` rustflags,
  lowers it in-process, and writes to the shared cargo target dir:
  `target/groundhog/<snake>.rv32a` (artifact), `<snake>.rv32.json` (metadata
  sidecar with `code_bytes`, `initial_memory_bytes`, `memory_bytes`,
  `artifact_bytes`, hashes, build flags), and `<snake>.spec.{json,xdr}`.
  The unstripped ELF stays at the usual cargo path, so the existing
  dispatch/mem-symbol/string checks keep working unchanged.
- Validated 2026-07-09 with CLI 0.5.7 against the dev SDK (pin since bumped
  to 0.5.14): `counter-rv32`
  (example-contracts) and `itc_erc20` (integration_test) both build and lower
  end-to-end with `CARGO_PROFILE_RELEASE_STRIP=false` and
  `-- -Zbuild-std=core,alloc`.
- Metrics (`scripts/metrics.sh`, main-branch CI only) currently produce
  `sdk_codesizes` (wasm `ls -la`), `sdk_codesizes_rv32` (readelf sections of
  the raw ELF), and `sdk_perf_gas`. Metric history is append-only.

## Design

### 1. dev.sh rv32 builds go through the CLI

`build_rv32()` switches to:

```sh
CARGO_PROFILE_RELEASE_STRIP=false \
groundhog build --target rv32 -p "$package" -- -Zbuild-std=core,alloc
```

run from the contract workspace directory, exactly as the cargo invocation
is today. Wasm builds are unchanged (cargo + wasm-opt + in-tree
groundhog-spec-tool), so SDK/spec-format co-evolution is not blocked by the
pinned CLI on the wasm path.

Unchanged invariants:

- ELF path: `target/riscv32im-unknown-none-elf/release/<snake>` — the
  dispatch, mem-symbol, and embedded-string checks run on it as before.
- The `*-rv32` wrapper discovery convention in example-contracts.

New outputs per rv32 contract: `target/groundhog/<snake>.rv32a`,
`<snake>.rv32.json`, `<snake>.spec.json`, `<snake>.spec.xdr`. Note
`target/groundhog/` is shared by both contract workspaces (both set
`target-dir = "../target"`); package names must stay unique across the two
rv32 sets (they are today; the metric script fails loudly on duplicates).

### 2. integration_test rv32 starter set

New list file `integration_test/rv32-contracts` — one package name per
line, comments/blank lines allowed — is the single source of truth for
which integration_test contracts get rv32 builds. Initial contents:

```
rv32_smoke_counter
itc_erc20
fibonacci
```

(Nearly every integration_test contract already carries an rv32 wrapper
`main.rs`, so a name/marker convention cannot select the enabled set; an
explicit list can grow one line at a time.)

- New `itc_build_rv32_all` in dev.sh: for each listed package, CLI rv32
  build + `check_rv32_dispatch` + `check_rv32_mem_symbols`, mirroring
  `eg_build_rv32_all`. Wired into `check_all`'s INTEGRATION block and given
  a `build-integration-rv32` subcommand.
- `rv32_smoke_counter` is added to the integration_test workspace members.
  Its explicit `groundhog-rv32-runtime` dependency is dropped — the runtime
  arrives transitively via `sdk` on rv32 targets (the counter-rv32 pattern) —
  so host `cargo test --workspace` keeps compiling. Its `disabled` marker
  stays, keeping it out of the wasm build and lint loops.

### 3. New metric: sdk_codesizes_rv32_lowered

`scripts/metrics/sdk_codesizes_rv32_lowered.sh <groundhog artifact dir>`:

- Input set derived from the source tree, never by globbing the artifact
  dir (same stale-artifact rationale documented in `sdk_codesizes_rv32.sh`):
  example-contracts `*-rv32` package glob plus the
  `integration_test/rv32-contracts` list.
- For each package, parse `target/groundhog/<snake>.rv32.json` and emit one
  line (append-only format — do not reorder or rename fields):

  ```
  <snake> code=<B> initial_memory=<B> memory=<B> artifact=<B>
  ```

  where the fields are the sidecar's `code_bytes`,
  `initial_memory_bytes`, `memory_bytes`, `artifact_bytes`.
- Missing sidecar or duplicate name → loud failure (exit 1), never a
  zero-valued or partial line.
- `scripts/metrics.sh` adds
  `produce_metric sdk_codesizes_rv32_lowered "target/groundhog"`.

The existing `sdk_codesizes_rv32` metric is unchanged in format and
semantics (per-section ELF detail) and now also covers the integration_test
rv32 set, deriving those names from the same list file.

### 4. CLI version pin in CI shell scripts

- `scripts/groundhog-cli-version.sh` holds the single pin:
  `GROUNDHOG_CLI_VERSION=0.5.14`.
- `scripts/ci.sh` sources it and, before `dev.sh ci`, runs:

  ```sh
  cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
      --registry groundhog --locked
  ```

  (No-op when that exact version is already installed. The registry index
  and CI token are already configured; `sdk_tests.yml` skips examples and
  integration so it needs no CLI.)
- Optionally add `~/.cargo/bin` to ci.yml's Namespace cache list so
  main-branch runs skip the one-time CLI compile.
- dev.sh behavior (local runs): if `groundhog` is absent, fail with the
  install command; if present at a different version, warn once and
  continue with the PATH version. CI is authoritative for metrics.

## Trade-offs / risks

- **Pinned-CLI vs dev-SDK skew**: a released CLI lowers ELFs built from the
  in-tree SDK. An SDK change to the artifact/ABI contract blocks CI until a
  new CLI release and a one-line pin bump. Accepted — dogfooding the
  released CLI against the dev SDK is part of the point.
- **CLI build flags differ slightly from the old invocation**: the CLI
  passes `--no-default-features --bin <snake>`. No current rv32 package
  declares default features, and every rv32 package already declares the
  snake-named bin; both verified in the trial builds.
- **Metric continuity**: the new metric starts fresh; the existing rv32
  metric's history gains integration_test rows (new names, existing fields —
  fine for an append-only per-name history).

## Testing

- `./scripts/dev.sh build-examples-rv32` and `build-integration-rv32`
  locally: ELF checks pass, `.rv32a` + `.rv32.json` present for every
  package in both input sets.
- Run `scripts/metrics/sdk_codesizes_rv32_lowered.sh target/groundhog`
  directly: one well-formed line per contract; delete one sidecar and
  confirm loud failure.
- `cd integration_test && cargo test --workspace` still passes with
  rv32_smoke_counter as a member.
- Full `./scripts/dev.sh ci` (with the usual skips) green.
