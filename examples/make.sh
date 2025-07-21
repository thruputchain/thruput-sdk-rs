#!/bin/bash

[ -z "${HOGD_SRC}" ] && HOGD_SRC="../hogd"

source ./cargo.sh

declare -a example_contracts
for d in * ; do
    [ -d "$d" ] || continue;
    [ ! -f "$d/disabled" ] || continue;

    example_contracts+=("$d")
done

function ls_contracts {
    for contract in "${example_contracts[@]}" ; do
        echo "$contract"
    done
}

function build_contract {
    contract="${1:?Please specify contract to build}"
    release="../target/wasm32-unknown-unknown/release/${contract}.wasm"
    optimized="../target/optimized/${contract}.wasm"

    cargo build \
        --features "guest" \
        --target wasm32-unknown-unknown \
        --release \
        --package "$contract" \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features=panic_immediate_abort \
        &&
    (cd ../target && mkdir -p optimized) &&
    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only very slightly better)
    wasm-opt -O3 --enable-bulk-memory -o "${optimized}" "${release}"
}

function build_contracts {
    for contract in "${example_contracts[@]}" ; do
        if ! build_contract "$contract" ; then
            echo
            echo ">> ${contract}: Build failed"
            exit 1
        fi
    done
}

function expand_contract {
    contract=${1:?Please specify contract to expand}
    shift

    cargo expand \
        --features "guest" \
        --target wasm32-unknown-unknown \
        --release \
        --package "$contract" \
        "$@"
}

function expand_test_contract {
    contract=${1:?Please specify contract to expand}
    shift

    cargo_native expand \
        --features "mock" \
        --package "$contract" \
        "$@"
}

function test_contract {
    contract=${1:?Please specify contract to build}

    cargo_native test \
            --package "$contract" \
            -- --nocapture
}

function test_contracts {
    for contract in "${example_contracts[@]}" ; do
        if ! test_contract "$contract" ; then
            echo
            echo ">> ${contract}: Test failed"
            exit 1
        fi
    done
}

what=${1:?Please specify what to do}
shift

case "$what" in
    build)
        if [ -n "$1" ]; then
            build_contract "$1"
        else
            build_contracts
        fi
        ;;
    expand)
        expand_contract "$1"
        ;;
    expand-test)
        expand_test_contract "$1"
        ;;
    test)
        if [ -n "$1" ]; then
            test_contract "$1"
        else
            test_contracts
        fi
        ;;
    ls)
        ls_contracts
        ;;
    ls-wasms)
        ls -l ../target/optimized/*.wasm
        ;;
    ls-wasms-unoptimized)
        ls -l ../target/wasm32-unknown-unknown/release/*.wasm
        ;;
    * )
        echo "Sorry, didn't understand what to do."
        exit 1
        ;;
esac
