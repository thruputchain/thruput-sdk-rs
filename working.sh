#!/bin/bash

set -eu

build() {
    package="$1"; shift

    ./build-contract.sh "$package"
}

native_test() {
    package="$1"; shift

    cargo test -p "$package" "$@" -- --show-output
}

run_working() {
    ## This somehow negates settings in contract.toml?
    # export RUSTFLAGS="--deny warnings"

    # cargo test --lib -F dev
    # cargo test -F dev

    # something very strange is happening, which causes e.g. erc20
    # to have a much larger size when running this script than
    # when using build-contract.sh ... and then the executor chokes
    # on the big contract.

    (cd example-contracts && 
        cargo build --release --workspace)
    exit 0

    build erc20
    build erc20_no_storage
    build erc20_low_level
    build caller
    build exceptions

    native_test erc20
    native_test erc20_no_storage
    native_test erc20_low_level
    native_test caller --test one
    native_test caller --test two  # uses wasm compiled above

    native_test exceptions --test native
    # native_test exceptions --test wasm # uses compiled wasm

    # bottle tests?
}


run_working
