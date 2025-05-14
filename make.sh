#!/bin/bash

verb=${1:?}
shift

cargo="cargo +nightly"

case "$verb" in
    doc)
        $cargo doc --features=std "$@"
        ;;
    examples)
        # e.g. "examples build erc20"
        (cd examples && bash make.sh "$@")
        ;;
    test)
        $cargo test --jobs=1 --features=mock
        ;;
    check)
        $cargo check --features=std
        ;;
    check_guest)
        $cargo check --features=std,guest
        ;;
    lint)
        $cargo clippy --no-deps --features=std
        ;;
    lint_mock)
        $cargo clippy --no-deps --features=mock
        ;;
    lint_macros)
        $cargo clippy --no-deps -p macros
        ;;
    fmt)
        $cargo fmt --package \
            macros \
            groundhog_xdr \
            protocol_macros \
            native_exec \
            "$(ls examples | grep -v Makefile)"
        ;;
    tree)
        # Show the dependency graph
        cargo +nightly tree \
            --edges normal,build \
            --target wasm32-unknown-unknown
        ;;
    *)
        printf "Sorry, didn't understand what to do.\n"
        exit 1
        ;;
esac
