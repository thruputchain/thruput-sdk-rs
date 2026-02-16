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
    eg_test caller --test one
    eg_test caller --test two  # uses wasm compiled above
    eg_test erc20
    eg_test erc20_no_storage
    eg_test erc20_low_level
    # eg_test exceptions --test native
    # eg_test exceptions --test wasm # uses compiled wasm
    eg_test hasher
}

run_working() {
    # cargo test --lib -F dev
    # cargo test -F dev

    build_examples
    test_examples
}

run_working
echo "OK!"
