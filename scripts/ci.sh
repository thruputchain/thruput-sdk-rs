#!/bin/bash

set -u

# These only work if this script is run in the SDK's top-level (workspace) directory.
SDK_DIR="$(realpath ".")"
TARGET_DIR="$(realpath "target")"

### BUILDING ###

CONTRACT="${TARGET_DIR}/wasm32-unknown-unknown/release"
OPTIMIZED="${TARGET_DIR}/optimized"

build_contracts() {
    local contracts_dir
    contracts_dir="$1"; shift

    local package
    for package in $(ls_contract_packages "$contracts_dir") ; do
        build_contract "$contracts_dir" "$package" ||
            return 1
    done
}

build_contract() {
    local contracts_dir
    contracts_dir="$1"; shift
    local package
    package="$1"; shift

    echo && echo "-- BUILD $contracts_dir/$package"

    build_wasm "$contracts_dir" "$package" --release &&
    make_optimized "$package"
}

build_wasm() {
    local contracts_dir
    contracts_dir="$1"; shift
    local package
    package="$1"; shift

    (
        cd "$contracts_dir/$package" &&
        cargo --config "../.cargo/contract.toml" \
            build "$@"
    )
}

make_optimized() {
    local package
    package="$1"; shift

    local contract optimized
    contract="${CONTRACT}/${package}.wasm"
    optimized="${OPTIMIZED}/${package}.wasm"

    mkdir -p "$OPTIMIZED" &&

    debugging="-g" # XXX

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 "$debugging" --enable-bulk-memory -o "${optimized}" "${contract}"
}

expand_contract() {
    local package
    package="$1"; shift

    (
        cd "$package" &&
        cargo --config "${SDK_DIR}/.cargo/contract.toml" \
            expand \
            --features "sdk/std" \
            "$@"
    )
}

eg_build() {
    local package
    package="$1"; shift

    build_contract "${SDK_DIR}/example-contracts" "${package}"
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

    build_contract "${SDK_DIR}/integration_test_contracts" "${package}"
}

itc_build_all() {
    itc_build fibonacci &&
    build_contracts "${SDK_DIR}/integration_test_contracts"
}

### TESTING ###

test_sdk() {
    cargo test --lib &&
    cargo test --tests &&
    cargo test -F storage --tests &&
    true
}

test_contract() {
    local contracts_dir
    contracts_dir="$1"; shift
    local package
    package="$1"; shift

    echo && echo "-- TEST $contracts_dir/$package"
    (
        cd example-contracts &&
        cargo test -p "$package" "$@" -- --show-output
    )
}

eg_test() {
    local package
    package="$1"; shift

    contracts_dir="example-contracts"

    test_contract "$contracts_dir" "$package" "$@"
}

itc_test() {
    local package
    package="$1"; shift

    contracts_dir="integration_test_contracts"

    test_contract "$contracts_dir" "$package"
}

eg_test_all() {
    # Note: Some of these tests require their associated wasms to be compiled.

    eg_test caller --test native &&
    eg_test caller --test wasm &&
    eg_test erc20 &&
    eg_test erc20_no_storage &&
    eg_test erc20_low_level &&
    # eg_test exceptions --test native native_exceptions &&
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
        cargo test \
            --workspace  \
            --exclude txs_roundtrip \
            --exclude cxs_roundtrip \
            --exclude read_only_test \
            "$@" -- --show-output
    )
}

### BOTTLE TEST ###

BOTTLE_TEST_RAW="${TARGET_DIR}/wasm32-unknown-unknown/bottle_test"
BOTTLE_TEST="${TARGET_DIR}/bottle-test"

bottle_test_contract() {
    # NOTE: This requires that the contract's wasm has already been built

    local contracts_dir package
    contracts_dir="$1"; shift
    package="$1"; shift

    echo && echo "-- BOTTLE-TEST $contracts_dir/$package"

    build_wasm "$contracts_dir" "$package" \
        --profile="bottle_test" \
        --features="bottle-test" \
        -Z unstable-options \
        #

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
            [ ! -f "$contracts_dir/$package/disabled-bottle" ] || continue

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

cargo doc --features=std,storage,abort/debug
test_sdk

eg_build_all
eg_test_all
eg_bottle_test_all

itc_build_all
itc_test_all
itc_bottle_test_all

echo
echo "OK!"
