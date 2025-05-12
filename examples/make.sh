#!/bin/bash

cargo="cargo +nightly"

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
    contract=${1:?Please specify contract to build}

    $cargo build \
        --features "guest" \
        --target wasm32-unknown-unknown \
        --release \
        --verbose \
        --package "$contract" \
        --color=always \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features=panic_immediate_abort
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

function test_contract {
    contract=${1:?Please specify contract to build}

    HOGD="../../hogd"
    if [ ! -d "$HOGD" ]; then
        echo "Couldn't find hogd sources"
        exit 1
    fi

    CC=gcc CXX=g++ HOGD_SRC="$HOGD" \
        $cargo -v test \
            --color=always \
            --package "$contract"
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
        ls -l ../target/wasm32-unknown-unknown/release/*.wasm
        ;;
    * )
        echo "Sorry, didn't understand what to do."
        exit 1
        ;;
esac
