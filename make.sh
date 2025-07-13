#!/bin/bash

source examples/cargo.sh

verb=${1:?}
shift

case "$verb" in
    doc)
        $cargo doc --features=std "$@"
        ;;
    examples)
        # e.g. "examples build erc20"
        (cd examples && bash make.sh "$@")
        ;;
    integration_test_contracts)
        # e.g. "integration_test_contracts build erc20"
        (cd integration_test_contracts && bash make.sh "$@")
        ;;
    test)
        cargo_native test --features=mock "$@"
        ;;
    expand)
        cargo_native expand --features=mock "$@"
        ;;
    check)
        $cargo check --features=mock "$@"
        ;;
    lint)
        $cargo clippy --no-deps --features=std "$@"
        ;;
    lint_mock)
        cargo_native clippy --no-deps --features=mock "$@"
        ;;
    lint_macros)
        $cargo clippy --no-deps -p macros "$@"
        ;;
    fmt)
        $cargo fmt --package \
            macros \
            groundhog_xdr \
            protocol_macros \
            native_exec \
            "$(cd examples && bash make.sh ls)" \
            "$@"
        ;;
    tree)
        # Show the dependency graph
        $cargo tree \
            --target wasm32-unknown-unknown \
            --edges normal,build \
            "$@"
        ;;
    *)
        printf "Sorry, didn't understand what to do.\n"
        exit 1
        ;;
esac
