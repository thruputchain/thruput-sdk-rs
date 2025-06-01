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

## Documentation

To generate and open the SDK documentation:

```shell
./make.sh doc --open
```

## Testing

```shell
./make.sh test
./make.sh test_examples
```

## Development

See `make.sh` for various useful targets, including:
- `check`
- `test`
- `lint`
- `doc`
- `fmt`

### Expand macros to view syscall bindings

```shell
./make.sh expand syscall::bindings
```

### Expand macros to view example contract exports

```shell
./make.sh expand -p swap swapv2_pair::swap_exports
./make.sh expand -p swap swapv2_pair::erc20_exports
```
