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

    info "BUILD $contracts_dir/$package"

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

    # debugging="-g" # don't strip symbols?
    debugging=""

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 $debugging --enable-bulk-memory -o "${optimized}" "${contract}"
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

    info "TEST $contracts_dir/$package"
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

    info "BOTTLE-TEST $contracts_dir/$package"

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

build_docs() {
    cargo doc --features=dev,storage,abort/debug "$@"
}

test_spec() {
    cargo run -p groundhog-erc20-if -F spec --bin show_spec >/dev/null &&
    (
        cd integration_test_contracts &&
        cargo run -p fibonacci -F spec --bin generate_spec_fibonacci -- /dev/null &&
        cargo run -p tuple_return -F spec --bin generate_spec_tuple_return -- /dev/null &&
        true
    )
}

test_all() {
    build_docs &&
    test_sdk &&
    test_spec &&

    eg_build_all &&
    eg_test_all &&
    eg_bottle_test_all &&

    itc_build_all &&
    itc_test_all &&
    itc_bottle_test_all &&

    info "All tests succeeded." ||
    fail "Some tests failed"
}

### Other dev utils

tree_contract() {
    local contract_dir
    contract_dir="$1"; shift

    (
        cd "$contract_dir" &&
        cargo --config ../.cargo/contract.toml \
            tree \
            --target "wasm32-unknown-unknown" \
            --edges no-proc-macro,no-build,no-dev \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size \
            "$@"
    )
}

expand_contract() {
    contract="$1"; shift

    (
        cd "$contract" &&
        cargo --config ../.cargo/contract.toml \
            expand \
            --features "sdk/std" \
            --color=always \
        #
    )
}

### Quality checks

todo() {
    rg -i --ignore-file .local_src_ignore TODO
}

xxx() {
    rg -i \
        --ignore-file .local_src_ignore \
        --ignore-file .xxx_ignore \
        XXX
}

xxx_check() {
    # capture exit code without triggering errexit
    xxx && rg=$? || rg=$?

    if [ $rg -ne 1 ]; then
        # Matches found, or error
        fail "XXX annotations found!"
    else
        return 0
    fi
}

lint() {
    cargo clippy -F dev "$@"
}

lint_check() {
    lint "$@" >& /dev/null ||
        fail "Trapped lint!"
}

fmt() {
    cargo fmt "$@"
}

fmt_check() {
    fmt --check --quiet "$@" ||
        fail "Formatting required!"
}

ci_check() {
    # xxx_check &&
    fmt_check &&
    # lint_check &&
    test_all &&

    info "CI clean!" ||
    fail "CI failed!"
}

info() {
    printf "\n-- %s\n" "$1" >&2
}

warn() {
    printf "\n>> %s\n" "$1" >&2
}

fail() {
    printf "\n!! %s\n" "$1" >&2
    return 1
}

action="$1"; shift

case "$action" in
    expand)
        expand_contract "$1"
        ;;
    lint)
        lint
        ;;
    [xX][xX][xX])
        xxx
        ;;
    ci)
        ci_check
        ;;
    ls-enabled)
        ls_contract_packages "$1"
        ;;
    build-examples)
        eg_build_all
        ;;
    build-itc)
        itc_build_all
        ;;
    test)
        test_all
        ;;
    test-examples)
        eg_test_all
        ;;
    test-itc)
        itc_test_all
        ;;
    test-spec)
        test_spec
        ;;
    run-bottle-tests)
        eg_bottle_test_all &&
            itc_bottle_test_all
        ;;
    tree)
        tree_contract "$1"
        ;;
    build-docs)
        build_docs
        ;;
    open-docs)
        build_docs --open
        ;;
    rg)
        rg -i --ignore-file .local_src_ignore "$@"
        ;;
    *)
        fail "Sorry, didn't understand action '$action'."
        ;;
esac
