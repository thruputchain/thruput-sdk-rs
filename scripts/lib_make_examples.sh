# shellcheck shell=bash

# This file is designed to be sourced by lib_make.sh

set -u

cargo_guest() {
    local cmd="$1"
    shift

    RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort -Clink-args=-zstack-size=30000" \
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
