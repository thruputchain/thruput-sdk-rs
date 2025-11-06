# shellcheck shell=bash

# This file is designed to be sourced by lib_make.sh

set -u

# cargo profile for production contracts
PRODUCTION_PROFILE="contract"

# Directories for built contracts
CONTRACT="${TARGET_DIR}/wasm32-unknown-unknown/contract"
OPTIMIZED="${TARGET_DIR}/optimized"
BOTTLE_TEST="${TARGET_DIR}/bottle-test"

cargo_wasm() {
    local cmd="$1"
    shift

    flags="${RUSTFLAGS:-} -Zunstable-options -Cpanic=immediate-abort -Clink-args=-zstack-size=30000"
    # echo "RUSTFLAGS=\"$flags\""

    RUSTFLAGS="$flags" \
    cargo "$cmd" \
        --target wasm32-unknown-unknown \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features="std/optimize_for_size" \
        "$@"
}

build_wasm() {
    local package
    package="$(basename "$1")"
    shift

    cargo_wasm build \
        --features "guest" \
        --profile "$PRODUCTION_PROFILE" \
        --package "$package" \
        "$@"
}

make_optimized_wasm() {
    local package
    package="$(basename "$1")"
    shift

    (cd "$TARGET_DIR" && mkdir -p optimized) || return 1

    local contract="${CONTRACT}/${package}.wasm"
    local optimized="${OPTIMIZED}/${package}.wasm"

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 --enable-bulk-memory -o "${optimized}" "${contract}"
}

build_contract() {
    local contract="$1"
    shift

    if ! build_wasm "$contract" "$@" ; then
        echo
        echo ">> ${contract}: Failed building wasm"
        return 1
    fi

    if ! make_optimized_wasm "$contract" ; then
        echo
        echo ">> ${contract}: Failed optimizing wasm"
        return 1
    fi
}

build_bottle_test() {
    local package
    package="$(basename "$1")"
    shift

    cargo_wasm build \
        --features "bottle-test" \
        --profile "$PRODUCTION_PROFILE" \
        --package "$package" \
        -Zunstable-options --artifact-dir="target/bottle-test/" \
        "$@"
}

bottle_test_contract() {
    local contract="$1"; shift

    if bool_true "${BOTTLE_TEST_USE_BUILT_TARGET:-}"; then
        echo "(Using already-built target contract for bottle-test.)"
    else
        # Build the contract wasm itself
        build_contract "$contract" || return 1;
    fi

    build_bottle_test "$contract" "$@" || return 1;

    local package
    package="$(basename "$contract")"

    cargo run -p bottle-test --profile dev "$package"
}

has_bottle_test() {
    local contract="$1"; shift
    local cargo_toml="$contract/Cargo.toml"

    grep "bottle-test" "$cargo_toml" >/dev/null
}

bottle_test_contracts() {
    local contracts_dir="$1"; shift

    for contract in $(ls_contracts "$contracts_dir") ; do
        if has_bottle_test "$contract"; then
            bottle_test_contract "$contract" "$@" ||
                return 1
        fi
    done
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

    cargo_wasm tree \
        --edges normal,build,no-proc-macro \
        --features "guest" \
        --package "$package" \
        "$@"
}

expand_contract() {
    local package
    package="$(basename "$1")"
    shift

    cargo_wasm expand \
        --features "guest" \
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

# Use bottle-test if it exists, else use native test
test_contract() {
    local contract="$1"; shift

    if has_bottle_test "$contract"; then
        bottle_test_contract "$contract" "$@"
    else
        native_test_contract "$contract" "$@"
    fi
}

native_test_contract() {
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

metric_contract() {
    local package
    package="$(basename "$1")"
    shift

    local metric="${1:?}"
    shift

    cargo_native run \
        --package "$package" \
        --features "mock,client" \
        --profile "test" \
        "$@" \
        --example "$metric" \
        -- "$@"
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

native_test_contracts() {
    local contracts_dir="$1"
    shift

    local contract
    for contract in $(ls_contracts "$contracts_dir") ; do
        native_test_contract "$contract" "$@" ||
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

ls_contract_packages() {
    local contracts_dir="$1"
    shift

    local contract
    for contract in "$contracts_dir"/* ; do
        [ -d "$contract" ] || continue;
        [ ! -f "$contract"/disabled ] || continue;

        local c
        c="$(basename "$contract")"

        echo "$c"
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
        native-test)
            if [[ -n "${1:-}" && "$1" != "-"* ]]; then
                local contract="$contracts_dir/$1" ; shift

                native_test_contract "$contract" "$@"
            else
                native_test_contracts "$contracts_dir" "$@"
            fi
            ;;
        bottle-test)
            if [[ -n "${1:-}" && "$1" != "-"* ]]; then
                local contract="$contracts_dir/$1" ; shift

                bottle_test_contract "$contract" "$@"
            else
                bottle_test_contracts "$contracts_dir" "$@"
            fi
            ;;
        metric)
            local contract="$contracts_dir/$1" ; shift

            metric_contract "$contract" "$@"
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
        entrypoints)
            local contract="$1" ; shift
            local wasm="${OPTIMIZED}/${contract}.wasm"

            wasm-objdump -x -j Function "$wasm" | rustfilt
            ;;
        bottle-test-entrypoints)
            local contract="$1" ; shift
            local wasm="${BOTTLE_TEST}/${contract}.wasm"

            wasm-objdump -x -j Function "$wasm" | rustfilt
            ;;
        llvm-lines)
            local contract="$1" ; shift

            # also do: cargo install cargo-llvm-lines
            cargo_wasm llvm-lines \
                -p "$contract" \
                --features "guest" \
                "$@"
            ;;
        fns)
            local contract="$1" ; shift
            local wasm="${CONTRACT}/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            # also do: cargo install rustfilt
            wasm-objdump -x -j Code "$wasm" | rustfilt
            ;;
        sections)
            local contract="$1" ; shift
            local wasm="${CONTRACT}/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            wasm-objdump -h "$wasm"
            ;;
        decompile)
            local contract="$1" ; shift
            local wasm="${CONTRACT}/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            # also do: cargo install rustfilt
            wasm-decompile "$wasm" | perl -pe 's/\W(ZN\w*)\W/<_$1>/g' | rustfilt
            ;;
        dominators)
            local contract="$1" ; shift
            local wasm="${CONTRACT}/${contract}.wasm"

            # cargo install twiggy
            # https://github.com/AlexEne/twiggy/blob/master/guide/src/SUMMARY.md
            twiggy dominators "$wasm"
            ;;
        top-fns)
            local contract="$1" ; shift
            local wasm="${CONTRACT}/${contract}.wasm"

            # cargo install twiggy
            # https://github.com/AlexEne/twiggy/blob/master/guide/src/SUMMARY.md
            twiggy top "$wasm"
            ;;
        ls)
            ls_contracts "$contracts_dir"
            ;;
        ls-packages)
            ls_contract_packages "$contracts_dir"
            ;;
        ls-wasms)
            ls -l "$OPTIMIZED"/*.wasm
            ;;
        ls-wasms-unoptimized)
            ls -l "$CONTRACT"/*.wasm
            ;;
        * )
            echo "Sorry, didn't understand action '$action'."
            return 1
            ;;
    esac
}
