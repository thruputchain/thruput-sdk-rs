# Contributing to the Thruput Rust SDK

This guide covers working on the SDK itself. To write a contract with it, see
the [README](README.md).

[AGENTS.md](AGENTS.md) has the repository map, validation steps and change
guidelines; read it before a larger change.

## Set up this repository

Install [Rust with rustup](https://rustup.rs/), Git, and a native C/C++ linker.
Clone the repository and run development commands from its root:

```sh
git clone https://github.com/thruputchain/thruput-sdk-rs.git
cd thruput-sdk-rs
rustup show active-toolchain
mkdir -p target
```

[rust-toolchain.toml](rust-toolchain.toml) pins the nightly compiler,
`rust-src`, and the `riscv32im-unknown-none-elf` target. Rustup installs
missing components from this file. Use the pin instead of a floating
`+nightly` override. See
[rustup toolchain files](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file).

Dependencies resolve from crates.io and need no credentials, and so does the
pinned Thruput CLI (`thruput` on crates.io); see
[Build guest contracts](#build-guest-contracts).

Native tests download the Thruput test executor on Linux x86_64, Linux ARM64,
and macOS ARM64. The download is public and needs no credentials, only `curl`
and `tar`. See [native executor setup](crates/native-exec/README.md) for
custom libraries and source builds.

The SDK's native tests work without the integration submodule. Integration
tests need it:

```sh
git submodule update --init --recursive
```

## Run a native contract test

The [counter contract](fixtures/counter/src/implementation.rs) is a
small fixture for learning the test flow. From the repository root:

```sh
cargo gnaw test counter -- --show-output
```

[gnaw](gnaw/src/lib.rs) is this repository's task runner, in its own
workspace; the `cargo gnaw` alias builds and runs it. Name a contract by its
package name or its directory, such as `fixtures/counter`. This runs
native tests, including increment, decrement, and underflow checks,
without building a guest binary. The
[contract manifest](fixtures/counter/Cargo.toml) enables native test
support through its development dependency.

## Build guest contracts

Guest builds need the pinned Thruput CLI. From a Bash shell in the repository
root, source the installer so its PATH changes remain active:

```bash
. ./scripts/install-thruput-cli.sh
```

The helper configures the registry index and installs the release or source
commit in [the CLI pin](scripts/thruput-cli-version.sh). A source pin also
needs SSH access to the CLI repository, Python 3.11 or newer, and its Rust
toolchain. The CLI and SDK must agree on the contract-spec format; the
[scaffold smoke test](scripts/scaffold-smoke.sh) creates a project, points it
at this checkout, and builds it through the pinned CLI.

The CLI needs an active configuration profile even for offline builds. If you
have not selected one, create a local profile:

```sh
thruput config new sdk-local --network local --use --global --quiet
```

`cargo gnaw setup` does both steps: it installs the pin and creates a profile
if none is active.

Build contracts by name or directory, or every fixture or integration
contract:

```sh
cargo gnaw build counter fibonacci
cargo gnaw run fixtures:rv32
cargo gnaw run itc:rv32
```

The last command needs the integration submodule. Builds use the flags in
[the fixtures workspace config](fixtures/.cargo/config.toml) and its
[integration counterpart](integration_test/.cargo/config.toml), and write
lowered artifacts and specs under `target/thruput/`. Each build also checks
the linked ELF: the dispatch table, the runtime's `memcpy` override, and that
no source paths are embedded.

Some CI checks need more tools: `rg` (ripgrep) for the annotation check, and
`strings` for the storage-registry check. The benchmark and metrics scripts
need `llvm-readelf` or `readelf`.

## Run CI locally

CI runs gnaw tasks. Run the full CI sequence, including documentation, tests,
guest builds, artifact checks, linting, and formatting:

```sh
./scripts/ci.sh
```

This installs the pinned CLI if needed and creates a CLI profile if none is
active, then runs `cargo gnaw run ci`. Without the integration submodule, run
`./scripts/ci.sh quick sdk fixtures` and report that coverage was skipped.

Each CI job runs one gnaw group. Name tasks, groups, or globs to run part of
it; prerequisites are added unless you pass `--no-deps`:

| Command | Scope |
| --- | --- |
| `cargo gnaw list` | Tasks, with what each needs; `--groups` for groups |
| `cargo gnaw run quick` | Formatting, annotations, version pins, SDK clippy |
| `cargo gnaw run fixtures:test` | Fixture tests, after the RV32 builds they need |
| `cargo gnaw run 'fixtures-negative:*' -k` | Negative fixtures, reporting every failure |
| `cargo gnaw run sdk --dry-run` | The plan, without running it |

Task output goes to `target/gnaw/logs/`. A failure names the step that failed,
the command it ran, and how to rerun it. Pass `--unlocked` to let cargo update
lockfiles.

For single contracts, gnaw adds the flags guest builds need:

| Command | Scope |
| --- | --- |
| `cargo gnaw test <contract>... [-- <test args>]` | Native tests, one package at a time |
| `cargo gnaw build <contract>...` | RV32 build and ELF checks |
| `cargo gnaw lint <contract>...` | Guest and host clippy |
| `cargo gnaw expand <contract>` | Guest macro expansion; requires `cargo-expand` |
| `cargo gnaw tree <contract>` | Guest dependency tree |

Other development commands:

| Command | Scope |
| --- | --- |
| `cargo gnaw run sdk:test` | SDK test sequence |
| `cargo gnaw run lint` | SDK and contract lint checks |
| `cargo doc --no-deps -F dev --open` | Rustdoc for the workspace |
| `./scripts/check-docsrs.sh` | Each published crate's docs, built as docs.rs builds them |
| `cargo gnaw test bigint-bench-contract` | Host checks for arithmetic benchmark backends |
| `./scripts/bigint-bench.sh -o 'div sqrt'` | RV32 size and gas comparison for selected operations |

The full `bigint-bench.sh` sweep takes minutes and is separate from CI. Some
contract tests need built RV32 artifacts; full CI orders the build and test
steps. The root, fixtures, integration, negative fixtures, and logging
workspaces are separate. Root Cargo commands use `default-members` to exclude
the guest-only runtime from host builds.

## Diagnostics

For test diagnostics, set `SDK_TEST_ENABLE_SYSCALL_LOGGING=1` to log syscalls,
or `TEST_FFI_TRACE=1` to trace the native executor. Include a relevant trace
when reporting an execution bug.

## Pull requests

Keep each pull request to one change, with commits scoped to it. Say which
checks you ran and which you skipped. Documentation-only changes need link,
command and diff checks rather than full CI. API behavior belongs in rustdoc,
setup in this guide, and regression expectations in tests.
