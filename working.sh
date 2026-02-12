#!/bin/bash

set -eu

build() {
    package="$1"; shift

    ./build-contract.sh "$package"
}

native_test() {
    package="$1"; shift

    (cd example-contracts &&
        cargo test -p "$package" "$@" -- --show-output)
}

run_working() {
    # cargo test --lib -F dev
    # cargo test -F dev

    # build example contracts (wasm)
    (cd example-contracts && 
        cargo build \
            --workspace \
            --target "wasm32-unknown-unknown" \
            --profile release \
            -Z unstable-options \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size
    )

    # native tests not working yet
    exit 0

    native_test erc20
    native_test erc20_no_storage
    native_test erc20_low_level
    native_test caller --test one
    native_test caller --test two  # uses wasm compiled above
    native_test exceptions --test native
    # native_test exceptions --test wasm # uses compiled wasm
}

run_working
