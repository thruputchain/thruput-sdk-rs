# Thruput Rust SDK

Write smart contracts in Rust for Thruput's RV32 runtime. The SDK provides
contract macros, generated clients, persistent storage types, typed exceptions,
events, and a native test environment.

## Start a contract

For a workflow without the CLI, follow the tested
[Cargo-only example](crates/sdk/doc/cargo-only.md). It covers a standalone
manifest, native tests, and an RV32 ELF build, including the deployment boundary.

Use the CLI scaffold to create a contract outside this SDK checkout:

```sh
thruput new my-contract
cd my-contract
thruput build
```

The scaffold supplies the manifest, contract library, RV32 binary wrapper, and
pinned Rust toolchain. Keep its `entrypoints` feature and `src/main.rs` wrapper:
they link the contract's entrypoints into the deployed artifact. Native tests
use `dev`, which forwards to `thruput-sdk/dev`.

For persistent state, start with the executable
[multi-transaction tests](example-contracts/complex-storage/tests/test.rs).
The [event tests](crates/sdk/tests/events.rs) show typed event
checks, and [`Value`](crates/sdk/src/storage/value.rs) documents missing-value
handling.

See [Getting Started](https://docs.thruput.network/start/getting-started/) for
CLI bundles, registry access, and network setup, and
[thruput-examples](https://github.com/thruputchain/thruput-examples) for worked
contracts. [The SDK guide](crates/sdk/doc/lib.md) explains the generated crate,
features, storage, calls, and errors.

For development against this checkout, install the CLI pin as described below.
The CLI and SDK must agree on the contract-spec format. The
[scaffold smoke test](scripts/scaffold-smoke.sh) creates a project, points it
at this SDK checkout, and builds it through the pinned CLI.

## Set up this repository

Install [Rust with rustup](https://rustup.rs/), Git, and a native C/C++ linker.
Clone the repository and run development commands from its root:

```sh
git clone https://github.com/thruputchain/thruput-sdk-rs.git
cd thruput-sdk-rs
rustup show active-toolchain
mkdir -p target
```

[rust-toolchain.toml](rust-toolchain.toml) pins the nightly compiler, `rust-src`,
and the `riscv32im-unknown-none-elf` target. Rustup installs missing components
from this file. Use the pin instead of a floating `+nightly` override. See
[rustup toolchain files](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file).

Dependencies use the `thruput` registry configured in
[.cargo/config.toml](.cargo/config.toml). Builds need access to that registry.
Configure `CARGO_REGISTRIES_THRUPUT_TOKEN` through your Cargo credential setup
when authentication is required.

Native tests download the test executor on Linux x86_64, Linux ARM64, and
macOS ARM64. The download is public and needs no credentials, only `curl` and
`tar`; it does not need a node source checkout or ccache. See
[native executor setup](crates/native-exec/README.md) for custom libraries and
source builds. Source builds need CMake,
a C++ toolchain, libclang, and ccache. The pinned runtime source uses the
`groundhog` registry alias: set `CARGO_REGISTRIES_GROUNDHOG_TOKEN` to the same
value as `CARGO_REGISTRIES_THRUPUT_TOKEN` for that path. If using `HOGD_SRC`,
choose a checkout outside this SDK tree to keep Cargo configuration separate.

The SDK's native tests work without the private integration submodule.
Integration tests additionally require SSH access to its repository:

```sh
git submodule update --init --recursive
```

## Run a native contract test

The [counter contract](example-contracts/counter/src/implementation.rs) is a
small fixture for learning the test flow:

```sh
./scripts/dev.sh test example-contracts/counter -- --show-output
```

This runs native tests, including increment, decrement, and underflow checks.
It does not require building a guest binary first. The
[contract manifest](example-contracts/counter/Cargo.toml) enables native test
support through its development dependency, and the
[library entry point](example-contracts/counter/src/lib.rs) calls
`thruput_sdk::declare_mock_contract!()` to generate `register_mock_contract`.

For a single transaction, `thruput_sdk::client_test` deploys a registered
contract and passes its generated client to a closure. For example, in a
counter integration test:

```rust
#[test]
fn increment_counter() {
    thruput_sdk::client_test(
        |env| counter::register_mock_contract(env),
        |client: &counter::CounterClient| {
            client.initialize(10);
            client.increment(5);
            assert_eq!(client.get(), 15);
        },
    );
}
```

Use `TestEnvironment::transact` for multiple transactions, multiple contracts,
or frame-level exception and fault assertions. The
[counter tests](example-contracts/counter/tests/test.rs) show deployment and
error inspection with `try_decrement`. For RV32 execution, frame results expose
`rv32_stack_usage()` and `rv32_stack_usage_records()`; see
[frame results](crates/native-exec/src/frame_result.rs) for their scope.

`example-contracts/` is an internal test corpus. Individual fixtures exercise
SDK behavior and are not audited application templates.

## Build guest contracts

Guest builds need the pinned Thruput CLI. From a Bash shell in the repository
root, source the installer so its PATH changes remain active:

```bash
. ./scripts/install-thruput-cli.sh
```

The helper configures the registry index and installs the release or source
commit in [the CLI pin](scripts/thruput-cli-version.sh). A source pin also needs
SSH access to the CLI repository, Python 3.11 or newer, and its Rust toolchain.

The CLI needs an active configuration profile even for offline builds. If you
have not selected one, create a local profile:

```sh
thruput config new sdk-local --network local --use --global --quiet
```

For artifact checks, install `llvm-readelf` or `readelf`, an ELF-capable `nm`
(such as `llvm-nm`), and `rg` (ripgrep). The scripts also use standard Unix
utilities, including `realpath` and `strings`.

```sh
./scripts/dev.sh build-examples-rv32
./scripts/dev.sh build-integration-rv32
```

The second command needs the integration submodule. Builds use the flags in
[the example workspace config](example-contracts/.cargo/config.toml) and its
[integration counterpart](integration_test/.cargo/config.toml), and write
lowered artifacts and specs under `target/thruput/`.

## Develop the SDK

Run the full CI sequence, including documentation, tests, guest builds,
artifact checks, linting, and formatting:

```sh
./scripts/ci.sh
```

This installs the pinned CLI if needed and creates a CLI profile if none is
active, then runs `dev.sh ci`. Without access to the integration submodule,
use `SKIP_INTEGRATION=1 ./scripts/ci.sh` and report that coverage was skipped.

| Command | Scope |
| --- | --- |
| `./scripts/dev.sh test` | SDK test sequence |
| `./scripts/dev.sh test example-contracts/counter` | One contract's native tests |
| `./scripts/dev.sh test-examples` | Enabled example packages, tested separately |
| `./scripts/dev.sh test-itc` | Integration workspace tests |
| `./scripts/dev.sh lint` | SDK and contract lint checks |
| `./scripts/dev.sh expand example-contracts/counter` | Macro expansion; requires `cargo-expand` |
| `./scripts/dev.sh bigint-bench-test` | Host checks for arithmetic benchmark backends |
| `./scripts/dev.sh bigint-bench -o 'div sqrt'` | RV32 size and gas comparison for selected operations |

The full `bigint-bench` sweep takes minutes and is separate from CI. Some
contract tests need built RV32 artifacts; full CI orders the build and test
steps. The root, example, integration, negative-contract, and logging workspaces
are separate.
Root Cargo commands use `default-members` to exclude the guest-only runtime
from host builds. See [AGENTS.md](AGENTS.md) for the repository map and change
guidelines.

## Arithmetic and token semantics

Contract release builds disable overflow checks. Integer operators, including
`U256` and `I256`, can wrap on-chain where a native test panics. Use checked
arithmetic for amounts; a passing native test does not establish guest overflow
behavior. See [SDK arithmetic](crates/sdk/doc/lib.md#arithmetic) for checked
operations, full-precision multiplication and division, and storage guards,
and [256-bit math](crates/u256/src/lib.rs) for the integer types.

The [ERC20 issuer example](example-contracts/erc20_issuer/README.md) adds optional
holder authorization and clawback. Its
[Stellar integration notes](example-contracts/erc20_issuer/stellar-integration.md)
cover SEP-41 semantics and bridge boundaries.

## API documentation and diagnostics

Generate and open rustdoc:

```sh
./scripts/dev.sh open-docs
```

Use `./scripts/dev.sh build-docs` to generate it without opening a browser.
API documentation comes from the crate sources. See
[SDK features](crates/sdk/Cargo.toml), [storage primitives](crates/sdk/src/storage/),
and [storage flags](crates/sdk/src/sys_result/flags.rs) for details.

For test diagnostics, set `SDK_TEST_ENABLE_SYSCALL_LOGGING=1` to log syscalls,
or `TEST_FFI_TRACE=1` to trace the native executor. Include a relevant trace
when reporting an execution bug.

## License

Copyright 2026 Stellar Development Foundation. Licensed under the Apache
License, Version 2.0. Each published crate includes the workspace
[LICENSE](LICENSE).
