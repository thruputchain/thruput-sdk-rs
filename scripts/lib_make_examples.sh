# shellcheck shell=bash

# This file is designed to be sourced by lib_make.sh

set -u

cargo_guest() {
    local cmd="$1"
    shift

    RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort" \
    cargo "$cmd" \
        --features "guest" \
        --target wasm32-unknown-unknown \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features="std/optimize_for_size" \
        "$@"
}

build_wasm() {
    local package
    package="$(basename "$1")"
    shift

    cargo_guest build \
        --release \
        --package "$package" \
        "$@"
}

make_compressed_wasm() {
    local package
    package="$(basename "$1")"
    shift

    (cd "$TARGET_DIR" && mkdir -p optimized) || return 1

    local release="${TARGET_DIR}/wasm32-unknown-unknown/release/${package}.wasm"
    local optimized="${TARGET_DIR}/optimized/${package}.wasm"

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 --enable-bulk-memory -o "${optimized}" "${release}"
}

build_contract() {
    local contract="$1"
    shift

    if ! build_wasm "$contract" "$@" ; then
        echo
        echo ">> ${contract}: Failed building wasm"
        return 1
    fi

    if ! make_compressed_wasm "$contract" ; then
        echo
        echo ">> ${contract}: Failed compressing wasm"
        return 1
    fi
}

build_contracts() {
    local contracts_dir="$1"
    shift

    local contract
    for contract in $(ls_contracts "$contracts_dir") ; do
        build_contract "$contract" "$@" ||
            return 1
    done
}

tree_contract() {
    local package
    package="$(basename "$1")"
    shift

    cargo_guest tree \
        --package "$package" \
        "$@"
}

expand_contract() {
    local package
    package="$(basename "$1")"
    shift

    cargo_guest expand \
        --release \
        --package "$package" \
        "$@"
}

expand_test_contract() {
    local package
    package="$(basename "$1")"
    shift

    cargo_native expand \
        --package "$package" \
        "$@"
}

test_contract() {
    local package
    package="$(basename "$1")"
    shift

    cargo_native test \
        --package "$package" \
        "$@" \
        -- --nocapture \
        ||
    {
        echo
        echo ">> ${contract}: Test failed"
        return 1;
    }
}

test_contracts() {
    local contracts_dir="$1"
    shift

    local contract
    for contract in $(ls_contracts "$contracts_dir") ; do
        test_contract "$contract" "$@" ||
            return 1
    done
}

ls_contracts() {
    local contracts_dir="$1"
    shift

    local contract
    for contract in "$contracts_dir"/* ; do
        [ -d "$contract" ] || continue;
        [ ! -f "$contract"/disabled ] || continue;

        echo "$contract"
    done
}

make_contracts() {
    local contracts_dir=${1}
    local action=${2}
    shift 2

    case "$action" in
        build)
            if [[ -n "${1:-}" && "$1" != "-"* ]]; then
                local contract="$contracts_dir/$1" ; shift

                build_contract "$contract" "$@"
            else
                build_contracts "$contracts_dir" "$@"
            fi
            ;;
        test)
            if [[ -n "${1:-}" && "$1" != "-"* ]]; then
                local contract="$contracts_dir/$1" ; shift

                test_contract "$contract" "$@"
            else
                test_contracts "$contracts_dir" "$@"
            fi
            ;;
        expand)
            local contract="$contracts_dir/$1" ; shift

            expand_contract "$contract" "$@"
            ;;
        expand-test)
            local contract="$contracts_dir/$1" ; shift

            expand_test_contract "$contract" "$@"
            ;;
        tree)
            local contract="$contracts_dir/$1" ; shift

            tree_contract "$contract" "$@"
            ;;
        ls)
            ls_contracts "$contracts_dir"
            ;;
        ls-wasms)
            ls -l "$TARGET_DIR"/optimized/*.wasm
            ;;
        ls-wasms-unoptimized)
            ls -l "$TARGET_DIR"/wasm32-unknown-unknown/release/*.wasm
            ;;
        * )
            echo "Sorry, didn't understand action '$action'."
            return 1
            ;;
    esac
}
