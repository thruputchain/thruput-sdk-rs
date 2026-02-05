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

### Testing

The CI script above includes various tests that may be run on their own:

```shell
# Test the SDK
./scripts/make.sh test

# Test the example contracts
./scripts/make.sh examples test

# Test the erc20 example contract
./scripts/make.sh examples test erc20
```

### Diagnostics

TODO: Change these logging things into runtime configs (available
    only with feature "dev").

In contracts compiled to wasm for testing (e.g. in "bottle tests"), panic
messages will be lost unless configured with `PANIC_LOGGING=yes`, which sends
panic messages to the `log` syscall. If the test environment is then configured
with `SDK_TEST_ENABLE_SYSCALL_LOGGING=yes`, these messages will be printed to
the terminal.

See `scripts/lib_make.sh` for other useful targets.

### Expand macros to view contract exports

```shell
./scripts/make.sh examples expand erc20
```
