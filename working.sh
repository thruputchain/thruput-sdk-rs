#!/bin/bash

set -eu

build() {
    package="$1"; shift

    ./build-contract.sh "$package"
}

eg_test() {
    package="$1"; shift

    (cd example-contracts &&
        cargo test -p "$package" "$@" -- --show-output)
}

build_examples() {
    build "caller"
    # build "counter"
    build "erc20"
    build "erc20_no_storage"
    build "erc20_low_level"
    build "exceptions"
    build "genesis_deployer"
    build "hasher"
    build "rcq_test"
    build "simple_wallet_old"
    build "test_bytestring"
}

test_examples() {
    # some tests require wasms compiled above

    eg_test caller --test native
    eg_test caller --test wasm
    eg_test erc20
    eg_test erc20_no_storage
    eg_test erc20_low_level
    eg_test exceptions --test native native_exceptions
    # eg_test exceptions --test native native_panic # segfault!
    eg_test exceptions --test wasm wasm_ok
    eg_test exceptions --test wasm wasm_deploy
    eg_test exceptions --test wasm wasm_exceptions
    # eg_test exceptions --test wasm wasm_panic # panic confusion
    eg_test hasher
}

run_working() {
    cargo test --lib

        # cargo test --tests
    cargo test --test aborts
    cargo test --test bytestrings
    cargo test --test delete
    cargo test --test events
    cargo test --test hasher
    cargo test --test int128
    cargo test --test internal_nnints
    cargo test --test macros
    # cargo test --test nnints
    cargo test --test root_error_codes
    # cargo test -F storage --test storage

    build_examples
    test_examples
}

run_working
echo "OK!"
