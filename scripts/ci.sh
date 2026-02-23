#!/bin/bash

set -u

# These only work if this script is run in the SDK's top-level (workspace) directory.
SDK_DIR="$(realpath ".")"
TARGET_DIR="$(realpath "target")"

### BUILDING ###

CONTRACT="${TARGET_DIR}/wasm32-unknown-unknown/release"
OPTIMIZED="${TARGET_DIR}/optimized"

build_contract() {
    local package
    package="$1"; shift

    (
        cd "$package" &&
        cargo --config "${SDK_DIR}/.cargo/contract.toml" \
            build \
            --release \
            --target "wasm32-unknown-unknown" \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size \
            "$@"
    )
}

make_optimized() {
    local contract optimized
    contract="${CONTRACT}/${package}.wasm"
    optimized="${OPTIMIZED}/${package}.wasm"

    mkdir -p "$OPTIMIZED" &&

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 --enable-bulk-memory -o "${optimized}" "${contract}"
}

eg_build() {
    local package
    package="$1"; shift

    build_contract "${SDK_DIR}/example-contracts/${package}"
}

eg_build_all() {
    eg_build "caller" &&
    # build "counter" && # for Mark to update
    eg_build "erc20" &&
    eg_build "erc20_no_storage" &&
    eg_build "erc20_low_level" &&
    eg_build "exceptions" &&
    eg_build "genesis_deployer" &&
    eg_build "hasher" &&
    eg_build "rcq_test" &&
    eg_build "simple_wallet_old" &&
    eg_build "test_bytestring" &&
    true
}

itc_build() {
    package="$1"; shift

    build_contract "${SDK_DIR}/integration_test_contracts/${package}"
}

itc_build_all() {
    itc_build "authclaim_test" &&
    itc_build "calldata_raw" &&
    true
}

### TESTING ###

test_sdk() {
    cargo test --lib &&
    cargo test --tests &&
    cargo test -F storage --tests &&
    true
}

eg_test() {
    local package
    package="$1"; shift

    (
        cd example-contracts &&
        cargo test -p "$package" "$@" -- --show-output
    )
}

itc_test() {
    local package
    package="$1"; shift

    (
        cd integration_test_contracts &&
        cargo test -p "$package" "$@" -- --show-output
    )
}

eg_test_all() {
    # Note: Some of these tests require their associated wasms to be compiled.

    eg_test caller --test native &&
    eg_test caller --test wasm &&
    eg_test erc20 &&
    eg_test erc20_no_storage &&
    eg_test erc20_low_level &&
    eg_test exceptions --test native native_exceptions &&
    # eg_test exceptions --test native native_panic && # segfault!
    eg_test exceptions --test wasm wasm_ok &&
    eg_test exceptions --test wasm wasm_deploy &&
    eg_test exceptions --test wasm wasm_exceptions &&
    # eg_test exceptions --test wasm wasm_panic && # panic confusion
    eg_test hasher &&
    true
}

itc_test_all() {
    (
        cd integration_test_contracts &&
        cargo test --workspace "$@" -- --show-output
    )
}

### BOTTLE TEST ###

BOTTLE_TEST_RAW="${TARGET_DIR}/bottle-test-raw"
BOTTLE_TEST="${TARGET_DIR}/bottle-test"

bottle_test_contract() {
    # NOTE: This requires that the contract's wasm has already been built

    local contracts_dir package package_dir
    contracts_dir="$1"; shift
    package="$1"; shift
    package_dir="${SDK_DIR}/${contracts_dir}/${package}"

    build_contract "$package_dir" \
        --features="bottle-test" \
        -Z unstable-options \
        --artifact-dir="$BOTTLE_TEST_RAW" &&

    mkdir -p "$BOTTLE_TEST" &&
    wasm-opt -O3 --enable-bulk-memory \
        -o "$BOTTLE_TEST/${package}.wasm" \
        "$BOTTLE_TEST_RAW/${package}.wasm" &&

    cargo run -p bottle-test --profile dev "$package"
}

has_bottle_test() {
    local contract cargo_toml
    contract="$1"; shift
    cargo_toml="$contract/Cargo.toml"

    grep -s "bottle-test" "$cargo_toml" >/dev/null
}

bottle_test_contracts() {
    local contracts_dir
    contracts_dir="$1"; shift

    local package
    for package in $(ls_contract_packages "$contracts_dir") ; do
        if has_bottle_test "$contracts_dir/$package"; then
            bottle_test_contract "$contracts_dir" "$package" "$@" ||
                return 1
        fi
    done
}

ls_contract_packages() {
    local contracts_dir
    contracts_dir="$1"; shift

    local contract
    for contract in "$contracts_dir"/* ; do
        [ -d "$contract" ] || continue;
        [ ! -f "$contract"/disabled ] || continue;

        c="$(basename "$contract")"

        echo "$c"
    done
}

eg_bottle_test_all() {
    bottle_test_contracts "example-contracts"
}

itc_bottle_test_all() {
    bottle_test_contracts "integration_test_contracts"
}

### MAIN ###

fail() {
  printf "\n!! CI-FAIL"
  if [ $# -gt 0 ]; then
    echo ": $@"
  else
    echo "!"
  fi
  printf "\n"
  exit 1
}

trap fail ERR;

test_sdk

eg_build_all
eg_test_all
eg_bottle_test_all

# itc_build_all
# itc_test_all
# itc_bottle_test_all

echo
echo "OK!"
