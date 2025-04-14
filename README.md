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

```
rustup +nightly target add wasm32-unknown-unknown
```

Also, install the rust sources so the build process can
recompile them in a more compact form to reduce code size:

```
rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
```

## Documentation

Use `make doc_open` to generate and open the SDK documentation.

## Development

See `Makefile` for these useful targets:
- `check`
- `test`
- `lint`
- `doc`
- `fmt`
