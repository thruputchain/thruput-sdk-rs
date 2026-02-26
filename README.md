This SDK facilitates writing contracts in Rust.

## This repository

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
rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
```

### Ccache

Install [ccache](https://ccache.dev/) with your package manager.

## Building

```shell
# Retrieve needed cargo packages before building in offline mode in next step
./hogd/scripts/fetch.sh

# Run the CI test to ensure everything builds; this also runs many tests
./scripts/ci.sh
```

## Documentation

To generate and open the SDK documentation:

```shell
./scripts/make.sh doc --open
```

## Development

### Cargo layout for contracts

The `example-contracts` sub-package can be cloned outside of the SDK to provide
a testbed for your contracts.  See, e.g., `example-contracts/caller` for the
necessary cargo configuration, testing setup, etc.

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
- Set `TEST_FFI_TRACE` to see interactions with the test executor
