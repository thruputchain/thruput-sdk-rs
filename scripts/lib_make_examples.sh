# shellcheck shell=bash

# This file is designed to be sourced by lib_make.sh

set -u

cargo_wasm() {
    local cmd="$1"
    shift

    RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort -Clink-args=-zstack-size=30000" \
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

build_bottle_test() {
    local package
    package="$(basename "$1")"
    shift

    cargo_wasm build \
        --features "bottle-test" \
        --release \
        --package "$package" \
        -Zunstable-options --artifact-dir="target/bottle-test/" \
        "$@"
}

bottle_test_contract() {
    local contract="$1"; shift

    if [ -z "${BOTTLE_TEST_USE_BUILT_TARGET:-}" ]; then
        build_contract "$contract" || return 1;
    else
        echo "(Using already-built target contract for bottle-test.)"
    fi

    build_bottle_test "$contract" "$@" || return 1;

    local package
    package="$(basename "$contract")"

    cargo run -p bottle-test "$package"
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

    cargo_guest tree \
        --edges normal,build,no-proc-macro \
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
    local contract="$1"; shift

    if [ -n "${PREFER_BOTTLE_TEST:-}" ] && has_bottle_test "$contract"; then
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
            local wasm="${TARGET_DIR}/optimized/${contract}.wasm"

            wasm-objdump -x -j Function "$wasm" | rustfilt
            ;;
        bottle-test-entrypoints)
            local contract="$1" ; shift
            local wasm="${TARGET_DIR}/bottle-test/${contract}.wasm"

            wasm-objdump -x -j Function "$wasm" | rustfilt
            ;;
        llvm-lines)
            local contract="$1" ; shift

            # also do: cargo install cargo-llvm-lines
            cargo_guest llvm-lines -p "$contract"
            ;;
        fns)
            local contract="$1" ; shift
            local unoptimized_wasm="${TARGET_DIR}/wasm32-unknown-unknown/release/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            # also do: cargo install rustfilt
            wasm-objdump -x -j Code "$unoptimized_wasm" | rustfilt
            ;;
        sections)
            local contract="$1" ; shift
            local unoptimized_wasm="${TARGET_DIR}/wasm32-unknown-unknown/release/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            wasm-objdump -h "$unoptimized_wasm"
            ;;
        decompile)
            local contract="$1" ; shift
            local unoptimized_wasm="${TARGET_DIR}/wasm32-unknown-unknown/release/${contract}.wasm"

            # get this utility at: https://github.com/WebAssembly/wabt
            # also do: cargo install rustfilt
            wasm-decompile "$unoptimized_wasm" | perl -pe 's/\W(ZN\w*)\W/<_$1>/g' | rustfilt
            ;;
        dominators)
            local contract="$1" ; shift
            local unoptimized_wasm="${TARGET_DIR}/wasm32-unknown-unknown/release/${contract}.wasm"

            # cargo install twiggy
            # https://github.com/AlexEne/twiggy/blob/master/guide/src/SUMMARY.md
            twiggy dominators "$unoptimized_wasm"
            ;;
        top-fns)
            local contract="$1" ; shift
            local unoptimized_wasm="${TARGET_DIR}/wasm32-unknown-unknown/release/${contract}.wasm"

            # cargo install twiggy
            # https://github.com/AlexEne/twiggy/blob/master/guide/src/SUMMARY.md
            twiggy top "$unoptimized_wasm"
            ;;
        ls)
            ls_contracts "$contracts_dir"
            ;;
        ls-packages)
            ls_contract_packages "$contracts_dir"
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
