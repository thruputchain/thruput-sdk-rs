#!/bin/bash

cargo="cargo +nightly"

function cargo_native {
    if [ -z "${HOGD_SRC}" ]; then
        HOGD_SRC="../hogd"
    fi
    HOGD_SRC=$(realpath "${HOGD_SRC}")

    if [ ! -d "${HOGD_SRC}" ]; then
        echo "Couldn't find hogd sources"
        exit 1
    fi

    # pull in build config
    SCRIPT_DIR="${HOGD_SRC}/scripts"
    # shellcheck source="../hogd/scripts/env.sh"
    source "${SCRIPT_DIR}/env.sh"

    CC="${CC}" CXX="${CXX}" HOGD_SRC="${HOGD_SRC}" \
        $cargo "$@"
}

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
    test)
        cargo_native test --jobs=1 --features=mock
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
        cargo_native clippy --no-deps --features=mock
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
            "$(cd examples && bash make.sh ls)"
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
