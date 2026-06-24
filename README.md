This SDK facilitates writing contracts in Rust.

## Contract Author Quickstart

Polished examples and tutorials are maintained outside this repository.
(The `example-contracts/` directory here is an internal test corpus for SDK
development, not a set of recommended starting points.)

Contract crates using the native test harness typically need:

```toml
[features]
# Required: the #[contract] macro gates guest entrypoint codegen on this.
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
sdk = { path = "../sdk_rs/crates/sdk" }
# Add `features = ["storage"]` if the contract uses sdk::storage types.

[dev-dependencies]
my-contract = { path = ".", features = ["dev"] }
```

Tests run contracts natively against a `TestEnvironment` (an in-process
groundhog instance). The canonical pattern is the fluent transaction API:
each `env.transact` call builds one transaction, `tx.call(&addr).run(...)`
executes a closure as a contract frame at that address, and the returned
result is asserted on:

```rust,ignore
use sdk::*;

static NONCE: SHA256Hash = SHA256Hash::new(hash_to_array!("my-test-nonce"));

let env = TestEnvironment::no_cost();
let test_contract = my_contract::register_mock_contract(&env);
let contract_hash = *test_contract.as_hash();

env.transact(|tx| {
    tx.call(&ContractAddress::named("test")).run(move || {
        let addr = deploy_contract(&NONCE, &contract_hash);
        let client = MyContractClient::new(&addr);
        // exercise the generated client here
    })
})
.assert_success();
```

(`sdk::declare_mock_contract!();` once in `src/lib.rs` generates
`register_mock_contract`.)

For the common single-transaction case, `sdk::client_test` is thin sugar
over that pattern — it deploys the contract and hands the closure a client:

```rust,ignore
sdk::client_test(
    |env| my_contract::register_mock_contract(env),
    |client: &MyContractClient| {
        // exercise the generated client here
    },
);
```

Use the full `env.transact` form when a test needs multiple transactions,
asserts on aborts/faults, or decodes typed results via
`sdk::into_root_call_result`.

Generated clients default to `FooClient` for `#[contract] trait Foo` and for
`#[contract] impl Foo { ... }`. Use `client = CustomClient` only when you want
to override that default.

## This Repository

Note that this repository contains
[git submodules](https://git-scm.com/book/en/v2/Git-Tools-Submodules).
To include their contents when cloning, use:

```shell
git clone --recurse-submodules ${REPO_URL}
```

To retrieve or update their contents in an existing cloned repository, use:

```shell
git submodule update --init --recursive
```

## Requirements

### Rust toolchain

You can use [rustup](https://rustup.rs/) to get a Rust toolchain.

Then install the WASM target:

```shell
rustup +nightly target add wasm32-unknown-unknown
```

Also, install the rust sources so the build process can
recompile them in a more compact form to reduce code size:

```shell
rustup component add rust-src --toolchain nightly
```

### Ccache

Install [ccache](https://ccache.dev/) with your package manager.

## Building

```shell
# Run the CI test to ensure everything builds; this also runs many tests
./scripts/ci.sh
```

## Documentation

To generate and open the SDK documentation:

```shell
./scripts/dev.sh open-docs
```

## Development

### Dependencies

Notes on dependencies in this repo:

These crates are all versioned the same (via workspace.version) and are
published to the registry at the same time, so their repo-local dependencies
(via workspace.dependencies) use exact versioning:

- `crates/sdk`
- `crates/spec-tool`
- `crates/contract-spec`
- `crates/contract-util`
- `crates/event`
- `crates/macros`
- `crates/native-exec`
- `crates/test-vectors`

These are also versioned the same as the above, but don't depend on
repo-local crates:

- `crates/hogd-sys`
- `crates/storage-cost`
- `crates/wasm-runtime`
- `crates/rv32-runtime`

These crates are published to the registry with their own versions, and each
has a caret-versioned dependency on the registered SDK package:

- `standard-interfaces/*`

These crates are not published (just used in this repo, or, for
integration_test, used under a different Cargo.toml elsewhere), so dependencies
to repo-local crates are specified simply as paths (no registry/version):

- `crates/bottle-test`
- `crates/bottle-test-if`
- `crates/sdk/tests/vector-tests`
- `standard-tests/*`
- `integration_test/*`
- `example-contracts/*`

### Testing

The script `scripts/dev.sh` offers various commands useful in development,
e.g.:

```shell
alias dev='./scripts/dev.sh'

dev build-examples

# Expand macros to view internals of contract wrappers, etc.
dev expand example-contracts/erc20
```

### Diagnostics

When running tests:
- Set `SDK_TEST_ENABLE_SYSCALL_LOGGING=1` to see a log of syscalls made
- Set `TEST_FFI_TRACE=1` to see interactions with the test executor. When
  reporting a bug, it's helpful to include a trace captured with
  `TEST_FFI_TRACE=1`.
