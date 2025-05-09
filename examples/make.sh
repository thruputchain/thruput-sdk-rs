#!/bin/bash

what=${1:?Please specify what to do}
shift

cargo="cargo +nightly"

example_contracts="$(ls | grep -v '.sh$')"

function build_contract {
    contract=${1:?Please specify contract to build}

    $cargo build \
        --features "guest" \
        --target wasm32-unknown-unknown \
        --release \
        --verbose \
        --package "$contract" \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features=panic_immediate_abort
}

function build_contracts {
    for contract in $example_contracts ; do
        if ! build_contract "$contract" ; then
            printf ">> %s: Build failed\n" "$contract"
            exit 1
        fi
    done
}

function test_contract {
    contract=${1:?Please specify contract to build}

    HOGD="../../hogd"
    if [ ! -d "$HOGD" ]; then
        printf "Couldn't find hogd sources"
        exit 1
    fi

    CC=gcc CXX=g++ HOGD_SRC="$HOGD" \
        $cargo -v test --package "$contract"
}

function test_contracts {
    for contract in $example_contracts ; do
        if ! test_contract "$contract" ; then
            printf ">> %s: Test failed\n" "$contract"
            exit 1
        fi
    done
}

case "$what" in
    all)  build_contracts ;;
    test) test_contracts ;;
    ls-wasms)
        ls -l ../target/wasm32-unknown-unknown/release/*.wasm
        ;;
    * )
        if [ -d "$what" ]; then
            build_contract "$what"
        else
            printf "Sorry, didn't understand what to do.\n"
            exit 1
        fi
        ;;
esac
