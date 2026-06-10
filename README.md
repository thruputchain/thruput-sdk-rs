This SDK facilitates writing contracts in Rust.

## Contract Author Quickstart

The fastest way to get a new contract crate working is to start from one of
the small examples in this repository:

- `example-contracts/stateless` for a contract with no persistent state
- `example-contracts/complex-storage` for a contract using `sdk::storage`

Contract crates using the native test harness typically need:

```toml
[features]
dev = ["sdk/dev"]

[dependencies]
sdk = { path = "../sdk_rs" }
# Add `features = ["storage"]` if the contract uses sdk::storage types.

[dev-dependencies]
my-contract = { path = ".", features = ["dev"] }
```

The shortest test path is to call `sdk::declare_mock_contract!();` once in
`src/lib.rs` (which generates `register_mock_contract`), then:

```rust,ignore
sdk::client_test(
    |env| my_contract::register_mock_contract(env),
    |client: &MyContractClient| {
        // exercise the generated client here
    },
);
```

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

### Cargo layout for contracts

The `example-contracts` sub-package can be cloned outside of the SDK to provide
a testbed for your contracts. For a small starting point, prefer
`example-contracts/stateless`; for storage-backed contracts, prefer
`example-contracts/complex-storage`.

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
