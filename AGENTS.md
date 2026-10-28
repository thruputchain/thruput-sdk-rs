# AGENTS.md

## Scope

This repository contains the Thruput Rust SDK, its procedural macros, the RV32
guest runtime, and native test infrastructure.

For contract authoring, start with [Start a contract](README.md#start-a-contract).
Read [CONTRIBUTING.md](CONTRIBUTING.md) for setup and development commands. Run
`./scripts/dev.sh` from the repository root. Use the toolchain declared in
[rust-toolchain.toml](rust-toolchain.toml).

## Repository map

| Path | Purpose |
| --- | --- |
| `crates/sdk/` | Public SDK: calls, exceptions, storage, authentication, and test helpers |
| `crates/macros/` | Contract, storage, event, and exception macros and code generation |
| `crates/native-exec/` | Native execution environment and transaction/frame builders |
| `crates/hogd-sys/` | Native test executor FFI and library acquisition |
| `crates/rv32-runtime/` | RV32 guest runtime |
| `crates/contract-spec/`, `crates/spec-tool/` | Contract specifications and artifact tooling |
| `crates/event/`, `crates/exn/`, `crates/u256/` | Event, exception, and integer types |
| `crates/storage-cost/` | Storage cost support |
| `crates/contract-util/`, `crates/domain-separator/` | Shared macro utilities and domain hashing |
| `crates/bigint-bench/`, `crates/bigint-bench-if/` | Integer arithmetic benchmarks |
| `crates/test-vectors/`, `crates/version-check/` | Shared test vectors and release consistency checks |
| `example-contracts/` | Internal contract test corpus; separate Cargo workspace |
| `negative-contracts/` | Compile-fail guest fixtures; separate Cargo workspace |
| `integration_test/` | Integration workspace; `contracts/` is a Git submodule |
| `standard-interfaces/`, `standard-impls/`, `standard-tests/` | Standard interfaces, reusable implementations, and shared tests |
| `logging-test/` | Logging build fixture; separate Cargo workspace |
| `scripts/`, `.github/workflows/` | Development commands and CI/release automation |

## Validation

- Run focused tests for the changed crate or contract first. For example,
  `./scripts/dev.sh test example-contracts/counter -- --show-output`.
- Use `./scripts/ci.sh` for full validation. It installs the pinned Thruput
  CLI, then runs `./scripts/dev.sh ci`. See CONTRIBUTING.md prerequisites first.
- Root Cargo commands use `default-members`. Do not replace them with
  `--workspace`: the guest runtime crate requires its own target.
- Example and integration workspaces have separate manifests and lockfiles.
  A root test run does not cover them. Some tests also require built contracts.
- Run example packages separately, as `test-examples` does. Combining packages
  can change behavior through Cargo feature unification.
- `dev.sh` locks dependencies by default. Set `CARGO_UNLOCK` only when an
  intentional dependency update requires it.
- `./scripts/dev.sh test` runs the SDK test sequence. Use `test-examples` and
  `test-itc` for the separate contract workspaces.
- Report which checks ran and any blockers. Documentation-only changes need
  link, command, and diff checks; they do not normally need full CI.

## Implementation constraints

- Use [crates/sdk/Cargo.toml](crates/sdk/Cargo.toml) as the feature reference.
  `dev` enables native testing support; guest builds must not enable `std`.
  Storage and contract specs do not require features named `storage` or `spec`.
- Contract crates expose an `entrypoints` feature for guest code generation.
  Native test registration uses `thruput_sdk::declare_mock_contract!()` and
  `thruput-sdk/dev`.
  Follow the current fixtures when changing macro output.
- Check storage semantics in [crates/sdk/src/storage/](crates/sdk/src/storage/).
  `NNInt::inc` enforces its floor at commit; `strict_inc` checks immediately.
  Vector appends update an exclusive length slot. Do not assume all storage
  operations commute.
- Contract release builds disable overflow checks. Use checked arithmetic for
  amounts; native tests alone do not establish guest overflow behavior.
- Keep RV32 flags in the leaf workspace configs. The example, integration, and
  negative-contract workspaces' configs must stay synchronized. Adding the same
  flags at the root duplicates linker arguments through Cargo config merging.
- For version changes, follow [Cargo.toml](Cargo.toml) and
  [crates/version-check/README.md](crates/version-check/README.md), including
  dependent versions in the separate workspaces.
- Preserve unrelated working-tree and submodule changes. Keep commits scoped
  to the requested work.

## Documentation and communication

Use short sentences and consistent technical terms. Keep contract-author
guidance in README, setup instructions in CONTRIBUTING.md, API behavior in
rustdoc, and regression expectations in tests. Link to
these sources instead of copying them into this file. Git history retains old
plans and review reports.

Use neutral, task-focused branch names, commits, and pull request text. Do not
add agent attribution unless requested. Frame code review findings as specific
questions supported by source evidence.
